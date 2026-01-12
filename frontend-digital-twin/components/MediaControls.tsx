import * as React from 'react';
import { Mic, MicOff, Video, VideoOff, Monitor, Circle, Square } from 'lucide-react';
import { useMediaStream } from '../hooks/useMediaStream';
import DraggableMediaPreview from './DraggableMediaPreview';

export default function MediaControls() {
  const { state, actions } = useMediaStream();

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
          `h-8 w-8 rounded-md flex items-center justify-center transition-colors ` +
          (disabled
            ? 'opacity-40 cursor-not-allowed'
            : active
              ? 'bg-[#5381A5] text-white'
              : 'bg-white/20 text-[#0b1b2b] hover:bg-white/30')
        }
      >
        {children}
      </button>
    );
  };

  return (
    <>
      {/* Floating control bar */}
      <div className="fixed bottom-4 right-4 z-50 flex items-center gap-2 rounded-lg border border-white/20 bg-white/10 p-2 shadow-lg backdrop-blur">
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

        <div className="mx-1 h-5 w-px bg-white/20" />

        <IconButton
          title={state.isRecording ? 'Stop recording' : 'Start recording'}
          active={state.isRecording}
          disabled={state.isUploading}
          onClick={() => (state.isRecording ? actions.stopRecording() : actions.startRecording())}
        >
          {state.isRecording ? <Square size={16} /> : <Circle size={16} />}
        </IconButton>

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

      {/* PiP preview */}
      {state.previewStream && state.activeVideoSource && (
        <DraggableMediaPreview stream={state.previewStream} activeVideoSource={state.activeVideoSource} />
      )}
    </>
  );
}

