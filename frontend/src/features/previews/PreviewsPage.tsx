import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { listPreviews, createPreview, resetPreview, restartPreview, teardownPreview } from './api';
import type { Preview } from '../../lib/types';

export function PreviewsPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const [previews, setPreviews] = useState<Preview[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!projectId) return;
    listPreviews(projectId)
      .then(setPreviews)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [projectId]);

  const handleCreate = async () => {
    if (!projectId) return;
    const preview = await createPreview(projectId);
    setPreviews([preview, ...previews]);
  };

  const handleReset = async (id: string) => {
    await resetPreview(id);
  };

  const handleRestart = async (id: string) => {
    await restartPreview(id);
  };

  const handleTeardown = async (id: string) => {
    await teardownPreview(id);
    setPreviews(previews.filter((p) => p.id !== id));
  };

  if (loading) {
    return <p>Loading previews...</p>;
  }

  return (
    <div>
      <h2 className="p-heading--3">Previews</h2>
      <button className="p-button--positive" onClick={handleCreate}>
        Create preview
      </button>
      <div className="p-card">
        <div className="p-card__content">
          <table className="p-table">
            <thead>
              <tr>
                <th>Status</th>
                <th>Mode</th>
                <th>Branch</th>
                <th>Created</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {previews.map((preview) => (
                <tr key={preview.id}>
                  <td><span className="p-label">{preview.status}</span></td>
                  <td>{preview.mode}</td>
                  <td>{preview.branch || '-'}</td>
                  <td>{preview.created_at}</td>
                  <td>
                    <button className="p-button--link" onClick={() => handleReset(preview.id)}>Reset</button>
                    <button className="p-button--link" onClick={() => handleRestart(preview.id)}>Restart</button>
                    <button className="p-button--link" onClick={() => handleTeardown(preview.id)}>Tear down</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
