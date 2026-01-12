import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { Twin } from '../types';

interface MemoryResult {
  id: string;
  timestamp: string;
  content: string;
  agent_id: string;
  risk_level: string;
  similarity: number;
  memory_type: string;
  metadata: Record<string, string>;
}

interface MemoryExplorerProps {
  activeTwin?: Twin;
  onClose?: () => void;
}

interface DeleteConfirmModalProps {
  isOpen: boolean;
  memoryId: string;
  memoryContent: string;
  onConfirm: () => void;
  onCancel: () => void;
}

const DeleteConfirmModal: React.FC<DeleteConfirmModalProps> = ({
  isOpen,
  memoryId,
  memoryContent,
  onConfirm,
  onCancel,
}) => {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg shadow-xl p-6 max-w-md w-full mx-4">
        <h3 className="text-lg font-bold text-[#0b1b2b] mb-4">Confirm Deletion</h3>
        <p className="text-sm text-[#5381A5] mb-2">
          Are you sure you want to delete this memory?
        </p>
        <div className="bg-[#f0f0f0] p-3 rounded mb-4">
          <p className="text-xs font-semibold text-[#0b1b2b] mb-1">Memory ID:</p>
          <p className="text-xs text-[#5381A5] font-mono">{memoryId}</p>
          <p className="text-xs font-semibold text-[#0b1b2b] mb-1 mt-2">Preview:</p>
          <p className="text-xs text-[#0b1b2b] line-clamp-2">
            {memoryContent.substring(0, 100)}...
          </p>
        </div>
        <div className="flex gap-3 justify-end">
          <button
            onClick={onCancel}
            className="px-4 py-2 bg-[#90C3EA] text-[#0b1b2b] rounded hover:bg-[#78A2C2] transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            className="px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600 transition-colors"
          >
            Delete
          </button>
        </div>
      </div>
    </div>
  );
};

const MemoryExplorer: React.FC<MemoryExplorerProps> = ({ activeTwin, onClose }) => {
  const [memories, setMemories] = useState<MemoryResult[]>([]);
  const [allMemories, setAllMemories] = useState<MemoryResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(1);
  const [pageSize] = useState(50);
  const [totalCount, setTotalCount] = useState(0);
  const [totalPages, setTotalPages] = useState(0);
  const [namespace, setNamespace] = useState(activeTwin?.settings.memoryNamespace || '');
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [deleteModal, setDeleteModal] = useState<{ isOpen: boolean; memoryId: string; content: string }>({
    isOpen: false,
    memoryId: '',
    content: '',
  });

  // NOTE: The original orchestrator instance may already be running on 8182.
  // We default to 8185 for dev so we can run a second instance without killing the first.
  const orchestratorUrl = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8185';

  const loadMemories = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const response = await fetch(`${orchestratorUrl}/v1/memory/list`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          namespace: namespace.trim() || '',
          page,
          page_size: pageSize,
          twin_id: activeTwin?.id || '',
        }),
      });

      if (!response.ok) {
        throw new Error(`Failed to load memories: ${response.statusText}`);
      }

      const data = await response.json();
      const loadedMemories = data.memories || [];
      setAllMemories(loadedMemories);
      setTotalCount(data.total_count || 0);
      setTotalPages(data.total_pages || 0);
    } catch (err) {
      console.error('[MemoryExplorer] Load error:', err);
      setError(err instanceof Error ? err.message : 'Failed to load memories');
    } finally {
      setLoading(false);
    }
  }, [namespace, page, pageSize, activeTwin?.id, orchestratorUrl]);

  const handleDeleteClick = (memoryId: string, content: string) => {
    setDeleteModal({
      isOpen: true,
      memoryId,
      content,
    });
  };

  const handleDeleteConfirm = async () => {
    const { memoryId } = deleteModal;
    if (!namespace.trim()) {
      setError('Namespace is required');
      setDeleteModal({ isOpen: false, memoryId: '', content: '' });
      return;
    }

    setDeletingId(memoryId);
    setError(null);
    setDeleteModal({ isOpen: false, memoryId: '', content: '' });

    try {
      const response = await fetch(`${orchestratorUrl}/v1/memory/delete`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          memory_id: memoryId,
          namespace: namespace.trim(),
        }),
      });

      if (!response.ok) {
        throw new Error(`Failed to delete memory: ${response.statusText}`);
      }

      const data = await response.json();
      if (!data.success) {
        throw new Error(data.error_message || 'Delete failed');
      }

      // Reload memories after deletion
      await loadMemories();
    } catch (err) {
      console.error('[MemoryExplorer] Delete error:', err);
      setError(err instanceof Error ? err.message : 'Failed to delete memory');
    } finally {
      setDeletingId(null);
    }
  };

  // Client-side search filtering
  const filteredMemories = useMemo(() => {
    if (!searchQuery.trim()) {
      return allMemories;
    }
    const query = searchQuery.toLowerCase();
    return allMemories.filter((memory) =>
      memory.content.toLowerCase().includes(query) ||
      memory.id.toLowerCase().includes(query) ||
      memory.agent_id.toLowerCase().includes(query) ||
      (memory.memory_type && memory.memory_type.toLowerCase().includes(query))
    );
  }, [allMemories, searchQuery]);

  // Auto-load on mount
  useEffect(() => {
    loadMemories();
  }, [loadMemories]);

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      return date.toLocaleString();
    } catch {
      return timestamp;
    }
  };

  const truncateContent = (content: string, maxLength: number = 100) => {
    if (content.length <= maxLength) return content;
    return content.substring(0, maxLength) + '...';
  };

  return (
    <div className="flex-1 flex flex-col bg-[#9EC9D9] overflow-hidden font-display text-[#0b1b2b]">
      <div className="p-6 border-b border-[#5381A5]/30 bg-[#90C3EA]">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <span className="material-symbols-outlined text-[#5381A5]">database</span>
            <h2 className="text-xl font-bold text-[#0b1b2b] uppercase tracking-tight">
              Neural Archive Explorer
            </h2>
          </div>
          {onClose && (
            <button
              onClick={onClose}
              className="px-4 py-2 bg-[#5381A5] text-white rounded hover:bg-[#3d6a8a] transition-colors"
            >
              Close
            </button>
          )}
        </div>

        <div className="space-y-4">
          <div className="flex items-center gap-4">
            <div className="flex-1">
              <label className="block text-sm font-semibold mb-2 text-[#0b1b2b]">
                Namespace
              </label>
              <input
                type="text"
                value={namespace}
                onChange={(e) => setNamespace(e.target.value)}
                onKeyPress={(e) => e.key === 'Enter' && loadMemories()}
                placeholder="Enter namespace (e.g., threat_intel_v24)"
                className="w-full px-4 py-2 border border-[#5381A5]/30 rounded bg-white text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
              />
            </div>
            <div className="mt-6">
              <button
                onClick={loadMemories}
                disabled={loading || !namespace.trim()}
                className="px-6 py-2 bg-[#5381A5] text-white rounded hover:bg-[#3d6a8a] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {loading ? 'Loading...' : 'Load Memories'}
              </button>
            </div>
          </div>
          {allMemories.length > 0 && (
            <div>
              <label className="block text-sm font-semibold mb-2 text-[#0b1b2b]">
                Search Memories
              </label>
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search by content, ID, agent, or type..."
                className="w-full px-4 py-2 border border-[#5381A5]/30 rounded bg-white text-[#0b1b2b] focus:outline-none focus:ring-2 focus:ring-[#5381A5]"
              />
            </div>
          )}
        </div>

        {error && (
          <div className="mt-4 p-3 bg-red-100 border border-red-400 text-red-700 rounded">
            {error}
          </div>
        )}
      </div>

      <div className="flex-1 overflow-auto p-6">
        {loading && memories.length === 0 ? (
          <div className="text-center py-12 text-[#5381A5]">
            <span className="material-symbols-outlined text-4xl mb-2">hourglass_empty</span>
            <p>Loading memories...</p>
          </div>
        ) : memories.length === 0 ? (
          <div className="text-center py-12 text-[#5381A5]">
            <span className="material-symbols-outlined text-4xl mb-2">inbox</span>
            <p>No memories found. Try a different namespace or load memories.</p>
          </div>
        ) : (
          <>
            <div className="mb-4 text-sm text-[#5381A5]">
              {searchQuery.trim() ? (
                <>Showing {filteredMemories.length} of {allMemories.length} memories (filtered)</>
              ) : (
                <>Showing {allMemories.length} of {totalCount} memories (Page {page} of {totalPages})</>
              )}
            </div>
            <div className="bg-white rounded-lg shadow-sm overflow-hidden">
              <table className="w-full">
                <thead className="bg-[#90C3EA] border-b border-[#5381A5]/30">
                  <tr>
                    <th className="px-4 py-3 text-left text-sm font-semibold text-[#0b1b2b] uppercase tracking-tight">
                      ID
                    </th>
                    <th className="px-4 py-3 text-left text-sm font-semibold text-[#0b1b2b] uppercase tracking-tight">
                      Namespace
                    </th>
                    <th className="px-4 py-3 text-left text-sm font-semibold text-[#0b1b2b] uppercase tracking-tight">
                      Snippet Preview
                    </th>
                    <th className="px-4 py-3 text-left text-sm font-semibold text-[#0b1b2b] uppercase tracking-tight">
                      Created At
                    </th>
                    <th className="px-4 py-3 text-left text-sm font-semibold text-[#0b1b2b] uppercase tracking-tight">
                      Actions
                    </th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-[#5381A5]/20">
                  {filteredMemories.map((memory) => (
                    <tr
                      key={memory.id}
                      className="hover:bg-[#9EC9D9]/20 transition-colors"
                    >
                      <td className="px-4 py-3 text-sm text-[#0b1b2b] font-mono text-xs">
                        {memory.id.substring(0, 8)}...
                      </td>
                      <td className="px-4 py-3 text-sm text-[#0b1b2b]">
                        {namespace || 'N/A'}
                      </td>
                      <td className="px-4 py-3 text-sm text-[#0b1b2b]">
                        <div className="max-w-md">
                          {truncateContent(memory.content, 80)}
                        </div>
                      </td>
                      <td className="px-4 py-3 text-sm text-[#5381A5]">
                        {formatTimestamp(memory.timestamp)}
                      </td>
                      <td className="px-4 py-3">
                        <button
                          onClick={() => handleDeleteClick(memory.id, memory.content)}
                          disabled={deletingId === memory.id}
                          className="p-2 text-red-500 hover:bg-red-50 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                          title="Delete memory"
                        >
                          {deletingId === memory.id ? (
                            <span className="material-symbols-outlined text-sm animate-spin">hourglass_empty</span>
                          ) : (
                            <span className="material-symbols-outlined text-sm">delete</span>
                          )}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {totalPages > 1 && (
              <div className="mt-6 flex items-center justify-center gap-2">
                <button
                  onClick={() => setPage((p) => Math.max(1, p - 1))}
                  disabled={page === 1 || loading}
                  className="px-4 py-2 bg-[#5381A5] text-white rounded hover:bg-[#3d6a8a] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Previous
                </button>
                <span className="px-4 py-2 text-[#0b1b2b]">
                  Page {page} of {totalPages}
                </span>
                <button
                  onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                  disabled={page === totalPages || loading}
                  className="px-4 py-2 bg-[#5381A5] text-white rounded hover:bg-[#3d6a8a] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Next
                </button>
              </div>
            )}
          </>
        )}
      </div>

      <DeleteConfirmModal
        isOpen={deleteModal.isOpen}
        memoryId={deleteModal.memoryId}
        memoryContent={deleteModal.content}
        onConfirm={handleDeleteConfirm}
        onCancel={() => setDeleteModal({ isOpen: false, memoryId: '', content: '' })}
      />
    </div>
  );
};

export default MemoryExplorer;
