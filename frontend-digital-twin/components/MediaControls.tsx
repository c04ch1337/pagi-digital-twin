import * as React from 'react';
import { Mic, MicOff, Video, VideoOff, Monitor, Circle, Square, Library } from 'lucide-react';
import { useMediaStream } from '../hooks/useMediaStream';
import DraggableMediaPreview from './DraggableMediaPreview';

interface MediaControlsProps {
  onOpenGallery?: () => void;
  placement?: 'floating' | 'header';
}

export default function MediaControls({ onOpenGallery, placement = 'floating' }: MediaControlsProps = {}) {
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
          `h-8 w-8 rounded-full flex items-center justify-center transition-colors ` +
          (disabled
            ? 'opacity-40 cursor-not-allowed'
            : active
              ? 'bg-[var(--bg-steel)] text-[var(--text-on-accent)]'
              : 'bg-[rgb(var(--surface-rgb)/0.15)] text-[var(--text-primary)] hover:bg-[rgb(var(--surface-rgb)/0.25)]')
        }
      >
        {children}
      </button>
    );
  };

  return (
    <>
      {/* Floating control bar */}
      <div
        className={
          placement === 'header'
            ? 'flex items-center gap-1.5 rounded-full border border-[rgb(var(--text-on-accent-rgb)/0.2)] bg-[rgb(var(--surface-rgb)/0.2)] px-2 py-1 shadow-sm backdrop-blur-md'
            : 'fixed bottom-4 right-4 z-50 flex items-center gap-1.5 rounded-full border border-[rgb(var(--text-on-accent-rgb)/0.2)] bg-[rgb(var(--surface-rgb)/0.1)] px-2 py-1.5 shadow-lg backdrop-blur-md'
        }
      >
        <IconButton
          title={state.micEnabled ? 'Mute microphone' : 'Enable microphone'}
          active={state.micEnabled}
          onClick={() => (state.micEnabled ? actions.disableMic() : actions.enableMic())}
        >
          {state.micEnabled ? <Mic size={14} /> : <MicOff size={14} />}
        </IconButton>

        <IconButton
          title={state.cameraEnabled ? 'Disable camera' : 'Enable camera'}
          active={state.cameraEnabled}
          onClick={() => (state.cameraEnabled ? actions.disableCamera() : actions.enableCamera())}
        >
          {state.cameraEnabled ? <Video size={14} /> : <VideoOff size={14} />}
        </IconButton>

        <IconButton
          title={state.screenEnabled ? 'Stop screenshare' : 'Start screenshare'}
          active={state.screenEnabled}
          onClick={() => (state.screenEnabled ? actions.stopScreenShare() : actions.startScreenShare())}
        >
          <Monitor size={14} />
        </IconButton>

        <div className="mx-1 h-5 w-px bg-[rgb(var(--surface-rgb)/0.2)]" />

        {onOpenGallery && (
          <IconButton
            title="Open Neural Archive"
            active={false}
            onClick={() => {
              // Defensive: some environments swallow a click if the handler throws.
              // Emit a console log so this is diagnosable in prod/devtools.
              console.log('[MediaControls] Open Neural Archive clicked');
              try {
                onOpenGallery();
              } catch (err) {
                console.error('[MediaControls] Failed to open Neural Archive', err);
              }
            }}
          >
            <Library size={14} />
          </IconButton>
        )}

        <div className="mx-1 h-5 w-px bg-[rgb(var(--surface-rgb)/0.2)]" />

        <IconButton
          title={state.isRecording ? 'Stop recording' : 'Start recording'}
          active={state.isRecording}
          disabled={state.isUploading}
          onClick={() => (state.isRecording ? actions.stopRecording() : actions.startRecording())}
        >
          {state.isRecording ? (
            <Square size={14} className="text-[rgb(var(--text-on-accent-rgb)/0.95)]" />
          ) : (
            <Circle size={14} className="text-[var(--danger)]" />
          )}
        </IconButton>

        {/* High-contrast recording indicator (privacy) */}
        <div
          className={
            'ml-1 h-2 w-2 rounded-full transition-opacity ' +
            (state.isRecording ? 'bg-[rgb(var(--danger-rgb)/1)] animate-pulse opacity-100' : 'bg-[rgb(var(--surface-rgb)/0.3)] opacity-60')
          }
          title={state.isRecording ? 'Recording is ON' : 'Recording is OFF'}
          aria-label={state.isRecording ? 'Recording is ON' : 'Recording is OFF'}
        />

        <div className="ml-1 flex items-center gap-2">
          {state.isUploading && <span className="text-[10px] text-[rgb(var(--text-on-accent-rgb)/0.8)]">uploading…</span>}
          {state.error && (
            <span
              className="max-w-[220px] truncate text-[10px] text-[rgb(var(--danger-rgb)/0.9)]"
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
        <div
          className={
            placement === 'header'
              ? 'fixed top-16 right-4 z-50 rounded-lg border border-[rgb(var(--text-on-accent-rgb)/0.15)] bg-[rgb(var(--overlay-rgb)/0.6)] px-3 py-2 text-xs text-[var(--text-on-accent)] shadow-lg backdrop-blur-md'
              : 'fixed bottom-16 right-4 z-50 rounded-lg border border-[rgb(var(--text-on-accent-rgb)/0.15)] bg-[rgb(var(--overlay-rgb)/0.6)] px-3 py-2 text-xs text-[var(--text-on-accent)] shadow-lg backdrop-blur-md'
          }
        >
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

