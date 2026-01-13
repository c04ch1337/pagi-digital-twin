import React, { useState, useEffect } from 'react';
import { AgentCommand } from '../types/protocol';

interface CommandModalProps {
  command: AgentCommand | null;
  decisionTrace?: string | null;
  isVisible: boolean;
  onClose: () => void;
  onExecute: (command: AgentCommand, value: string) => void;
  onDeny: () => void;
}

const CommandModal: React.FC<CommandModalProps> = ({ 
  command, 
  decisionTrace,
  isVisible, 
  onClose, 
  onExecute, 
  onDeny 
}) => {
  const [inputValue, setInputValue] = useState('');
  const [isTraceOpen, setIsTraceOpen] = useState(false);

  // Reset input when modal opens/closes
  useEffect(() => {
    if (isVisible) {
      setInputValue('');
      setIsTraceOpen(false);
    }
  }, [isVisible, command]);

  const formattedDecisionTrace = (() => {
    if (!decisionTrace) return null;
    try {
      return JSON.stringify(JSON.parse(decisionTrace), null, 2);
    } catch {
      return decisionTrace;
    }
  })();

  if (!isVisible || !command) return null;

  const isConfigPrompt = command.command === 'prompt_for_config';
  const isToolExecution = command.command === 'execute_tool';
  const isMemoryPage = command.command === 'show_memory_page';

  // Determine prompt text based on command type
  let promptText = '';
  let title = '';
  
  if (isConfigPrompt) {
    title = 'Configuration Required';
    promptText = command.prompt || `Please provide a value for ${command.config_key || 'configuration'}`;
  } else if (isToolExecution) {
    title = 'Tool Execution Authorization';
    promptText = `The Digital Twin requires authorization to execute: **${command.tool_name}**\n\n${typeof command.arguments === 'object' && command.arguments !== null ? JSON.stringify(command.arguments, null, 2) : command.arguments || 'No additional arguments'}`;
  } else if (isMemoryPage) {
    title = 'Memory Page Request';
    promptText = `The agent wants to show memory page: ${command.memory_id}\nQuery: ${command.query}`;
  }

  const handleSubmission = () => {
    if (isConfigPrompt && !inputValue.trim()) {
      return; // Don't submit empty config values
    }
    
    // For tool execution, value is confirmation
    // For config prompt, value is user input
    const valueToSend = isConfigPrompt ? inputValue.trim() : 'CONFIRMED';
    
    onExecute(command, valueToSend);
    setInputValue('');
    onClose();
  };

  const handleDeny = () => {
    onDeny();
    setInputValue('');
    onClose();
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (isConfigPrompt && inputValue.trim()) {
        handleSubmission();
      } else if (!isConfigPrompt) {
        handleSubmission();
      }
    }
    if (e.key === 'Escape') {
      handleDeny();
    }
  };

  return (
    <div 
      className="fixed inset-0 bg-[#9EC9D9]/80 backdrop-blur-sm flex items-center justify-center z-50 animate-in fade-in duration-200"
      onClick={onClose}
    >
      <div 
        className="bg-[#90C3EA] border border-[#5381A5]/30 rounded-2xl p-6 w-full max-w-md shadow-2xl animate-in slide-in-from-bottom-4 duration-300"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-white/40 border border-[#5381A5]/30 flex items-center justify-center">
              <span className="material-symbols-outlined text-[#5381A5] text-xl">
                {isConfigPrompt ? 'settings' : isToolExecution ? 'construction' : 'database'}
              </span>
            </div>
            <h3 className="text-lg font-bold text-[#0b1b2b]">{title}</h3>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 hover:bg-[#78A2C2] rounded-lg transition-colors text-[#163247] hover:text-[#0b1b2b]"
          >
            <span className="material-symbols-outlined text-xl">close</span>
          </button>
        </div>

        {/* Content */}
        <div className="mb-6">
          <p className="text-[#163247] text-sm leading-relaxed whitespace-pre-wrap">
            {promptText}
          </p>
        </div>

        {/* Input for config prompts */}
        {isConfigPrompt && (
          <div className="mb-6">
            <label className="block text-xs font-semibold text-[#163247] uppercase tracking-wider mb-2">
              {command.config_key || 'Configuration Value'}
            </label>
            <input
              type="text"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              onKeyDown={handleKeyPress}
              placeholder={`Enter value for ${command.config_key || 'config'}...`}
              className="w-full bg-white/80 border border-[#5381A5]/30 rounded-lg px-4 py-2.5 text-sm text-[#0b1b2b] placeholder-[#163247]/70 focus:outline-none focus:ring-2 focus:ring-[#5381A5]/50 focus:border-[#5381A5]/50 transition-all"
              autoFocus
            />
          </div>
        )}

        {/* Tool execution details */}
        {isToolExecution && command.arguments && (
          <div className="mb-6 p-3 bg-white/60 border border-[#5381A5]/30 rounded-lg">
            <div className="text-xs font-semibold text-[#163247] uppercase tracking-wider mb-2">
              Tool Arguments
            </div>
            <pre className="text-xs text-[#0b1b2b] font-mono overflow-x-auto">
              {typeof command.arguments === 'object' 
                ? JSON.stringify(command.arguments, null, 2)
                : String(command.arguments)}
            </pre>
          </div>
        )}

        {/* AI Decision Trace */}
        {(isToolExecution || isMemoryPage) && formattedDecisionTrace && (
          <div className="mb-6">
            <button
              type="button"
              onClick={() => setIsTraceOpen(v => !v)}
              className="w-full flex items-center justify-between px-3 py-2 bg-white/60 border border-[#5381A5]/30 rounded-lg hover:bg-white/80 transition-colors"
            >
              <span className="text-xs font-semibold text-[#163247] uppercase tracking-wider">
                AI Decision Trace
              </span>
              <span className="material-symbols-outlined text-[#5381A5] text-lg">
                {isTraceOpen ? 'expand_less' : 'expand_more'}
              </span>
            </button>
            {isTraceOpen && (
              <pre className="mt-2 p-3 bg-white/40 border border-[#5381A5]/30 rounded-lg text-xs text-[#0b1b2b] font-mono overflow-x-auto whitespace-pre">
                {formattedDecisionTrace}
              </pre>
            )}
          </div>
        )}

        {/* Action Buttons */}
        <div className="flex gap-3">
          <button
            onClick={handleSubmission}
            disabled={isConfigPrompt && !inputValue.trim()}
            className="flex-1 bg-[#5381A5] hover:bg-[#437091] disabled:opacity-50 disabled:cursor-not-allowed text-white font-semibold py-2.5 px-4 rounded-lg transition-all flex items-center justify-center gap-2"
          >
            <span className="material-symbols-outlined text-lg">
              {isConfigPrompt ? 'check_circle' : 'play_arrow'}
            </span>
            <span className="text-sm">
              {isConfigPrompt ? 'Submit Value' : isToolExecution ? 'Authorize & Execute' : 'Show Memory'}
            </span>
          </button>
          <button
            onClick={handleDeny}
            className="px-4 py-2.5 bg-white/60 hover:bg-white/80 text-[#163247] font-semibold rounded-lg border border-[#5381A5]/30 transition-all flex items-center justify-center gap-2"
          >
            <span className="material-symbols-outlined text-lg">close</span>
            <span className="text-sm">Deny</span>
          </button>
        </div>

        {/* Keyboard hint */}
        <div className="mt-4 text-center">
          <p className="text-[10px] text-[#163247]/70">
            {isConfigPrompt ? 'Press Enter to submit, Esc to cancel' : 'Press Esc to cancel'}
          </p>
        </div>
      </div>
    </div>
  );
};

export default CommandModal;
