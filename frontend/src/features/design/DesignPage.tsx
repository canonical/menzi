import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { listClauses, listAmendments, approveAmendment, rejectAmendment } from './api';
import type { DesignClause, DesignAmendment } from '../../lib/types';

export function DesignPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const [clauses, setClauses] = useState<DesignClause[]>([]);
  const [amendments, setAmendments] = useState<DesignAmendment[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!projectId) return;
    Promise.all([listClauses(projectId), listAmendments(projectId)])
      .then(([c, a]) => {
        setClauses(c);
        setAmendments(a);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [projectId]);

  const handleApprove = async (amendmentId: string) => {
    if (!projectId) return;
    await approveAmendment(projectId, amendmentId);
    setAmendments(amendments.map((a) => a.id === amendmentId ? { ...a, status: 'approved' } : a));
  };

  const handleReject = async (amendmentId: string) => {
    if (!projectId) return;
    await rejectAmendment(projectId, amendmentId);
    setAmendments(amendments.map((a) => a.id === amendmentId ? { ...a, status: 'rejected' } : a));
  };

  if (loading) {
    return <p>Loading design...</p>;
  }

  return (
    <div>
      <h2 className="p-heading--3">Design</h2>
      <div className="p-card">
        <div className="p-card__header">
          <h3 className="p-heading--5">Clauses</h3>
        </div>
        <div className="p-card__content">
          <table className="p-table">
            <thead>
              <tr>
                <th>ID</th>
                <th>Title</th>
                <th>Level</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {clauses.map((clause) => (
                <tr key={clause.id}>
                  <td>{clause.clause_id}</td>
                  <td>{clause.title}</td>
                  <td><span className="p-label">{clause.level}</span></td>
                  <td><span className="p-label">{clause.status}</span></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
      <div className="p-card">
        <div className="p-card__header">
          <h3 className="p-heading--5">Amendments</h3>
        </div>
        <div className="p-card__content">
          <table className="p-table">
            <thead>
              <tr>
                <th>ID</th>
                <th>Action</th>
                <th>Status</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {amendments.map((amendment) => (
                <tr key={amendment.id}>
                  <td>{amendment.amendment_id}</td>
                  <td>{amendment.action}</td>
                  <td><span className="p-label">{amendment.status}</span></td>
                  <td>
                    <button className="p-button--link" onClick={() => handleApprove(amendment.id)}>Approve</button>
                    <button className="p-button--link" onClick={() => handleReject(amendment.id)}>Reject</button>
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
