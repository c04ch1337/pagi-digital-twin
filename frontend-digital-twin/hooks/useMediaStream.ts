import * as React from 'react';
import { usePagi } from '../context/PagiContext';

type UploadResult = {
  ok: boolean;
  status: number;
  bodyText?: string;
};

export type RecordingResult = {
  blob: Blob;
  mimeType: string;
  sizeBytes: number;
  startedAtMs: number;
  stoppedAtMs: number;
  upload?: UploadResult;
};

export type ActiveVideoSource = 'screen' | 'camera' | null;

export type MediaState = {
  micEnabled: boolean;
  cameraEnabled: boolean;
  screenEnabled: boolean;
  isRecording: boolean;
  isUploading: boolean;
  activeVideoSource: ActiveVideoSource;
  previewStream: MediaStream | null;
  lastRecording: RecordingResult | null;
  error: string | null;
};

function pickBestMimeType(hasVideo: boolean): string | undefined {
  const candidates = hasVideo
    ? [
        'video/webm;codecs=vp9,opus',
        'video/webm;codecs=vp8,opus',
        'video/webm;codecs=opus',
        'video/webm',
      ]
    : ['audio/webm;codecs=opus', 'audio/webm', 'audio/ogg;codecs=opus'];

  for (const t of candidates) {
    if (typeof MediaRecorder !== 'undefined' && MediaRecorder.isTypeSupported(t)) {
      return t;
    }
  }
  return undefined;
}

function stopStreamTracks(stream: MediaStream | null): void {
  if (!stream) return;
  stream.getTracks().forEach((t) => {
    try {
      t.stop();
    } catch {
      // ignore
    }
  });
}

export function useMediaStream() {
  const { currentUserId, sessionId } = usePagi();

  const userStreamRef = React.useRef<MediaStream | null>(null);
  const displayStreamRef = React.useRef<MediaStream | null>(null);
  const recorderRef = React.useRef<MediaRecorder | null>(null);
  const chunksRef = React.useRef<Blob[]>([]);
  const recordingStartedAtRef = React.useRef<number>(0);

  const [micEnabled, setMicEnabled] = React.useState(false);
  const [cameraEnabled, setCameraEnabled] = React.useState(false);
  const [screenEnabled, setScreenEnabled] = React.useState(false);
  const [isRecording, setIsRecording] = React.useState(false);
  const [isUploading, setIsUploading] = React.useState(false);
  const [activeVideoSource, setActiveVideoSource] = React.useState<ActiveVideoSource>(null);
  const [previewStream, setPreviewStream] = React.useState<MediaStream | null>(null);
  const [lastRecording, setLastRecording] = React.useState<RecordingResult | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  const uploadUrl = ((import.meta as any).env?.VITE_MEDIA_UPLOAD_URL as string | undefined)
    || 'http://127.0.0.1:8181/api/media/upload';

  const syncPreviewStream = React.useCallback(() => {
    const display = displayStreamRef.current;
    const user = userStreamRef.current;

    const screenTrack = display?.getVideoTracks()?.[0];
    const camTrack = user?.getVideoTracks()?.[0];

    const activeSource: ActiveVideoSource = screenEnabled && screenTrack
      ? 'screen'
      : cameraEnabled && camTrack
        ? 'camera'
        : null;

    setActiveVideoSource(activeSource);

    const trackToPreview = activeSource === 'screen' ? screenTrack : activeSource === 'camera' ? camTrack : undefined;
    if (!trackToPreview) {
      setPreviewStream(null);
      return;
    }

    // Use the same underlying track (do NOT stop this stream independently).
    setPreviewStream(new MediaStream([trackToPreview]));
  }, [cameraEnabled, screenEnabled]);

  const ensureUserStream = React.useCallback(async (): Promise<MediaStream> => {
    if (userStreamRef.current) return userStreamRef.current;

    setError(null);
    const stream = await navigator.mediaDevices.getUserMedia({ video: true, audio: true });
    userStreamRef.current = stream;
    return stream;
  }, []);

  const enableMic = React.useCallback(async () => {
    const stream = await ensureUserStream();
    stream.getAudioTracks().forEach((t) => (t.enabled = true));
    setMicEnabled(true);
  }, [ensureUserStream]);

  const disableMic = React.useCallback(() => {
    const stream = userStreamRef.current;
    stream?.getAudioTracks().forEach((t) => (t.enabled = false));
    setMicEnabled(false);
  }, []);

  const enableCamera = React.useCallback(async () => {
    const stream = await ensureUserStream();
    stream.getVideoTracks().forEach((t) => (t.enabled = true));
    setCameraEnabled(true);
  }, [ensureUserStream]);

  const disableCamera = React.useCallback(() => {
    const stream = userStreamRef.current;
    stream?.getVideoTracks().forEach((t) => (t.enabled = false));
    setCameraEnabled(false);
  }, []);

  const startScreenShare = React.useCallback(async () => {
    setError(null);
    const stream = await navigator.mediaDevices.getDisplayMedia({ video: true, audio: true });
    displayStreamRef.current = stream;
    setScreenEnabled(true);

    // If the user stops screensharing via the browser UI, reflect it in our state.
    stream.getVideoTracks().forEach((t) => {
      t.onended = () => {
        displayStreamRef.current = null;
        setScreenEnabled(false);
      };
    });
  }, []);

  const stopScreenShare = React.useCallback(() => {
    const stream = displayStreamRef.current;
    displayStreamRef.current = null;
    stopStreamTracks(stream);
    setScreenEnabled(false);
  }, []);

  const buildRecordingStream = React.useCallback((): { stream: MediaStream; hasVideo: boolean } => {
    const out = new MediaStream();

    const display = displayStreamRef.current;
    const user = userStreamRef.current;

    // Video: prefer screen, fallback to camera
    const screenTrack = screenEnabled ? display?.getVideoTracks()?.[0] : undefined;
    const camTrack = cameraEnabled ? user?.getVideoTracks()?.[0] : undefined;
    const videoTrack = screenTrack ?? camTrack;
    if (videoTrack) {
      out.addTrack(videoTrack);
    }

    // Audio: prefer mic, otherwise allow display audio (if present)
    const micTrack = micEnabled ? user?.getAudioTracks()?.[0] : undefined;
    const displayAudioTrack = screenEnabled ? display?.getAudioTracks()?.[0] : undefined;
    const audioTrack = micTrack ?? displayAudioTrack;
    if (audioTrack) {
      out.addTrack(audioTrack);
    }

    return { stream: out, hasVideo: Boolean(videoTrack) };
  }, [cameraEnabled, micEnabled, screenEnabled]);

  const uploadRecording = React.useCallback(
    async (blob: Blob, mimeType: string, startedAtMs: number, stoppedAtMs: number): Promise<UploadResult | undefined> => {
      if (!uploadUrl || uploadUrl.trim().length === 0) {
        return undefined;
      }

      setIsUploading(true);
      try {
        const ext = mimeType.includes('webm') ? 'webm' : mimeType.includes('ogg') ? 'ogg' : 'bin';
        const filename = `media_${currentUserId || 'unknown'}_${startedAtMs}.${ext}`;
        const file = new File([blob], filename, { type: mimeType || blob.type || 'application/octet-stream' });

        const form = new FormData();
        form.append('file', file);
        form.append('user_id', currentUserId || '');
        form.append('session_id', sessionId || '');
        form.append('started_at_ms', String(startedAtMs));
        form.append('stopped_at_ms', String(stoppedAtMs));
        form.append('mime_type', mimeType);

        const resp = await fetch(uploadUrl, {
          method: 'POST',
          body: form,
        });

        const bodyText = await resp.text().catch(() => undefined);
        return { ok: resp.ok, status: resp.status, bodyText };
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setError(`[media] upload failed: ${msg}`);
        return { ok: false, status: 0, bodyText: msg };
      } finally {
        setIsUploading(false);
      }
    },
    [currentUserId, sessionId, uploadUrl]
  );

  const startRecording = React.useCallback(() => {
    if (isRecording) return;
    if (recorderRef.current) return;

    setError(null);
    setLastRecording(null);

    const { stream, hasVideo } = buildRecordingStream();
    if (stream.getTracks().length === 0) {
      setError('[media] No active audio/video tracks to record. Enable Mic/Video/Screen first.');
      return;
    }

    const mimeType = pickBestMimeType(hasVideo);

    try {
      const recorder = new MediaRecorder(stream, mimeType ? { mimeType } : undefined);
      recorderRef.current = recorder;
      chunksRef.current = [];
      recordingStartedAtRef.current = Date.now();

      recorder.ondataavailable = (ev: BlobEvent) => {
        if (ev.data && ev.data.size > 0) {
          chunksRef.current.push(ev.data);
        }
      };

      recorder.onstop = async () => {
        const startedAtMs = recordingStartedAtRef.current;
        const stoppedAtMs = Date.now();
        const recordedMime = recorder.mimeType || mimeType || 'video/webm';
        const blob = new Blob(chunksRef.current, { type: recordedMime });

        recorderRef.current = null;
        chunksRef.current = [];
        setIsRecording(false);

        const upload = await uploadRecording(blob, recordedMime, startedAtMs, stoppedAtMs);
        setLastRecording({
          blob,
          mimeType: recordedMime,
          sizeBytes: blob.size,
          startedAtMs,
          stoppedAtMs,
          upload,
        });
      };

      recorder.start(1000);
      setIsRecording(true);
    } catch (e) {
      recorderRef.current = null;
      const msg = e instanceof Error ? e.message : String(e);
      setError(`[media] MediaRecorder failed: ${msg}`);
    }
  }, [buildRecordingStream, isRecording, uploadRecording]);

  const stopRecording = React.useCallback(() => {
    const recorder = recorderRef.current;
    if (!recorder) return;
    if (recorder.state === 'inactive') return;
    try {
      recorder.stop();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(`[media] stopRecording failed: ${msg}`);
    }
  }, []);

  // Keep preview derived from current enablement + streams.
  React.useEffect(() => {
    syncPreviewStream();
  }, [syncPreviewStream]);

  // If we disable both mic+camera, clean up user media tracks.
  React.useEffect(() => {
    const user = userStreamRef.current;
    if (!user) return;
    if (micEnabled || cameraEnabled) return;

    // We have a stream but both tracks are disabled — release devices.
    userStreamRef.current = null;
    stopStreamTracks(user);
  }, [micEnabled, cameraEnabled]);

  // On unmount, stop any captures.
  React.useEffect(() => {
    return () => {
      try {
        recorderRef.current?.stop();
      } catch {
        // ignore
      }
      recorderRef.current = null;

      stopStreamTracks(displayStreamRef.current);
      displayStreamRef.current = null;

      stopStreamTracks(userStreamRef.current);
      userStreamRef.current = null;
    };
  }, []);

  const state: MediaState = {
    micEnabled,
    cameraEnabled,
    screenEnabled,
    isRecording,
    isUploading,
    activeVideoSource,
    previewStream,
    lastRecording,
    error,
  };

  return {
    state,
    actions: {
      enableMic,
      disableMic,
      enableCamera,
      disableCamera,
      startScreenShare,
      stopScreenShare,
      startRecording,
      stopRecording,
      clearError: () => setError(null),
    },
  };
}

