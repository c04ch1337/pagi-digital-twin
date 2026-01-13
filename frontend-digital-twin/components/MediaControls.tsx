import * as React from 'react';
import { Mic, MicOff, Video, VideoOff, Monitor, Circle, Square, Library } from 'lucide-react';
import { useMediaStream } from '../hooks/useMediaStream';
import DraggableMediaPreview from './DraggableMediaPreview';

interface MediaControlsProps {
  onOpenGallery?: () => void;
}

export default function MediaControls({ onOpenGallery }: MediaControlsProps = {}) {
  const { state, actions } = useMediaStream();

  const [toast, setToast] = React.useState<string | null>(null);
  const toastTimerRef = React.useRef<number | null>(null);

  React.useEffect(() => {
    const upload = state.lastRecording?.upload;
    if (!upload?.ok) return;

    setToast('Neural Archive received recording.');
    if (toastTimerRef.current) {
      window.clearTimeout(toastTimerRef.current);
    }
    toastTimerRef.current = window.setTimeout(() => setToast(null), 2200);

    return () => {
      if (toastTimerRef.current) {
        window.clearTimeout(toastTimerRef.current);
        toastTimerRef.current = null;
      }
    };
  }, [state.lastRecording?.upload?.ok]);

  const IconButton: React.FC<{
    title: string;
    active?: boolean;
    disabled?: boolean;
    onClick: () => void;
    children: React.ReactNode;
  }> = ({ title, active, disabled, onClick, children }) => {
    return (
      <button
        type="button"
        title={title}
        disabled={disabled}
        onClick={onClick}
        className={
          `h-9 w-9 rounded-full flex items-center justify-center transition-colors ` +
          (disabled
            ? 'opacity-40 cursor-not-allowed'
            : active
              ? 'bg-[#5381A5] text-white'
              : 'bg-white/15 text-[#0b1b2b] hover:bg-white/25')
        }
      >
        {children}
      </button>
    );
  };

  return (
    <>
      {/* Floating control bar */}
      <div className="fixed bottom-4 right-4 z-50 flex items-center gap-2 rounded-full border border-white/20 bg-white/10 px-3 py-2 shadow-lg backdrop-blur-md">
        <IconButton
          title={state.micEnabled ? 'Mute microphone' : 'Enable microphone'}
          active={state.micEnabled}
          onClick={() => (state.micEnabled ? actions.disableMic() : actions.enableMic())}
        >
          {state.micEnabled ? <Mic size={16} /> : <MicOff size={16} />}
        </IconButton>

        <IconButton
          title={state.cameraEnabled ? 'Disable camera' : 'Enable camera'}
          active={state.cameraEnabled}
          onClick={() => (state.cameraEnabled ? actions.disableCamera() : actions.enableCamera())}
        >
          {state.cameraEnabled ? <Video size={16} /> : <VideoOff size={16} />}
        </IconButton>

        <IconButton
          title={state.screenEnabled ? 'Stop screenshare' : 'Start screenshare'}
          active={state.screenEnabled}
          onClick={() => (state.screenEnabled ? actions.stopScreenShare() : actions.startScreenShare())}
        >
          <Monitor size={16} />
        </IconButton>

        <div className="mx-1 h-6 w-px bg-white/20" />

        {onOpenGallery && (
          <IconButton
            title="Open Neural Archive"
            active={false}
            onClick={onOpenGallery}
          >
            <Library size={16} />
          </IconButton>
        )}

        <div className="mx-1 h-6 w-px bg-white/20" />

        <IconButton
          title={state.isRecording ? 'Stop recording' : 'Start recording'}
          active={state.isRecording}
          disabled={state.isUploading}
          onClick={() => (state.isRecording ? actions.stopRecording() : actions.startRecording())}
        >
          {state.isRecording ? (
            <Square size={16} className="text-red-50" />
          ) : (
            <Circle size={16} className="text-red-600" />
          )}
        </IconButton>

        {/* High-contrast recording indicator (privacy) */}
        <div
          className={
            'ml-1 h-2.5 w-2.5 rounded-full transition-opacity ' +
            (state.isRecording ? 'bg-red-500 animate-pulse opacity-100' : 'bg-white/30 opacity-60')
          }
          title={state.isRecording ? 'Recording is ON' : 'Recording is OFF'}
          aria-label={state.isRecording ? 'Recording is ON' : 'Recording is OFF'}
        />

        <div className="ml-1 flex items-center gap-2">
          {state.isUploading && <span className="text-[10px] text-white/80">uploading…</span>}
          {state.error && (
            <span
              className="max-w-[220px] truncate text-[10px] text-red-100"
              title={state.error}
              onClick={actions.clearError}
            >
              {state.error}
            </span>
          )}
        </div>
      </div>

      {/* Toast */}
      {toast && (
        <div className="fixed bottom-16 right-4 z-50 rounded-lg border border-white/15 bg-black/60 px-3 py-2 text-xs text-white shadow-lg backdrop-blur-md">
          {toast}
        </div>
      )}

      {/* PiP preview */}
      {state.previewStream && state.activeVideoSource && (
        <DraggableMediaPreview stream={state.previewStream} activeVideoSource={state.activeVideoSource} />
      )}
    </>
  );
}

