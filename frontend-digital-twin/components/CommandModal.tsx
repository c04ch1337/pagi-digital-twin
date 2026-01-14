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
      className="fixed inset-0 bg-[rgb(var(--bg-primary-rgb)/0.95)] backdrop-blur-md flex items-center justify-center z-[100] animate-in fade-in duration-200"
      onClick={onClose}
    >
      <div 
        className="bg-[var(--bg-secondary)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-2xl p-6 w-full max-w-md shadow-2xl animate-in slide-in-from-bottom-4 duration-300"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-lg bg-[rgb(var(--surface-rgb)/0.4)] border border-[rgb(var(--bg-steel-rgb)/0.3)] flex items-center justify-center">
              <span className="material-symbols-outlined text-[var(--bg-steel)] text-xl">
                {isConfigPrompt ? 'settings' : isToolExecution ? 'construction' : 'database'}
              </span>
            </div>
            <h3 className="text-lg font-bold text-[var(--text-primary)]">{title}</h3>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 hover:bg-[var(--bg-muted)] rounded-lg transition-colors text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
          >
            <span className="material-symbols-outlined text-xl">close</span>
          </button>
        </div>

        {/* Content */}
        <div className="mb-6">
          <p className="text-[var(--text-secondary)] text-sm leading-relaxed whitespace-pre-wrap">
            {promptText}
          </p>
        </div>

        {/* Input for config prompts */}
        {isConfigPrompt && (
          <div className="mb-6">
            <label className="block text-xs font-semibold text-[var(--text-secondary)] uppercase tracking-wider mb-2">
              {command.config_key || 'Configuration Value'}
            </label>
            <input
              type="text"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              onKeyDown={handleKeyPress}
              placeholder={`Enter value for ${command.config_key || 'config'}...`}
              className="w-full bg-[rgb(var(--surface-rgb)/0.8)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg px-4 py-2.5 text-sm text-[var(--text-primary)] placeholder-[rgb(var(--text-secondary-rgb)/0.7)] focus:outline-none focus:ring-2 focus:ring-[rgb(var(--bg-steel-rgb)/0.5)] focus:border-[rgb(var(--bg-steel-rgb)/0.5)] transition-all"
              autoFocus
            />
          </div>
        )}

        {/* Tool execution details */}
        {isToolExecution && command.arguments && (
          <div className="mb-6 p-3 bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg">
            <div className="text-xs font-semibold text-[var(--text-secondary)] uppercase tracking-wider mb-2">
              Tool Arguments
            </div>
            <pre className="text-xs text-[var(--text-primary)] font-mono overflow-x-auto">
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
              className="w-full flex items-center justify-between px-3 py-2 bg-[rgb(var(--surface-rgb)/0.6)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg hover:bg-[rgb(var(--surface-rgb)/0.8)] transition-colors"
            >
              <span className="text-xs font-semibold text-[var(--text-secondary)] uppercase tracking-wider">
                AI Decision Trace
              </span>
              <span className="material-symbols-outlined text-[var(--bg-steel)] text-lg">
                {isTraceOpen ? 'expand_less' : 'expand_more'}
              </span>
            </button>
            {isTraceOpen && (
              <pre className="mt-2 p-3 bg-[rgb(var(--surface-rgb)/0.4)] border border-[rgb(var(--bg-steel-rgb)/0.3)] rounded-lg text-xs text-[var(--text-primary)] font-mono overflow-x-auto whitespace-pre">
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
            className="flex-1 bg-[var(--bg-steel)] hover:bg-[rgb(var(--bg-steel-rgb)/0.85)] disabled:opacity-50 disabled:cursor-not-allowed text-[var(--text-on-accent)] font-semibold py-2.5 px-4 rounded-lg transition-all flex items-center justify-center gap-2"
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
            className="px-4 py-2.5 bg-[rgb(var(--surface-rgb)/0.6)] hover:bg-[rgb(var(--surface-rgb)/0.8)] text-[var(--text-secondary)] font-semibold rounded-lg border border-[rgb(var(--bg-steel-rgb)/0.3)] transition-all flex items-center justify-center gap-2"
          >
            <span className="material-symbols-outlined text-lg">close</span>
            <span className="text-sm">Deny</span>
          </button>
        </div>

        {/* Keyboard hint */}
        <div className="mt-4 text-center">
          <p className="text-[10px] text-[rgb(var(--text-secondary-rgb)/0.7)]">
            {isConfigPrompt ? 'Press Enter to submit, Esc to cancel' : 'Press Esc to cancel'}
          </p>
        </div>
      </div>
    </div>
  );
};

export default CommandModal;
