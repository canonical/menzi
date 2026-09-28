import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { getEnvironment, launchEnvironment, relaunchEnvironment, teardownEnvironment } from './api';
import type { Environment } from '../../lib/types';

export function EnvironmentPage() {
  const { workspaceId } = useParams<{ workspaceId: string }>();
  const [env, setEnv] = useState<Environment | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!workspaceId) return;
    getEnvironment(workspaceId)
      .then(setEnv)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [workspaceId]);

  const handleLaunch = async () => {
    if (!workspaceId) return;
    const result = await launchEnvironment(workspaceId, 'dev');
    setEnv(result);
  };

  const handleRelaunch = async (resetData: boolean) => {
    if (!workspaceId) return;
    const result = await relaunchEnvironment(workspaceId, resetData);
    setEnv(result);
  };

  const handleTeardown = async () => {
    if (!workspaceId) return;
    await teardownEnvironment(workspaceId);
    setEnv(null);
  };

  if (loading) {
    return <p>Loading environment...</p>;
  }

  return (
    <div>
      <h2 className="p-heading--3">Environment</h2>
      {env ? (
        <div className="p-card">
          <div className="p-card__header">
            <h3 className="p-heading--5">{env.name}</h3>
            <span className="p-label">{env.status}</span>
          </div>
          <div className="p-card__content">
            <h4>Components</h4>
            <ul className="p-list">
              {env.components.map((comp) => (
                <li key={comp.name} className="p-list__item">
                  {comp.name} - {comp.status} ({comp.health})
                </li>
              ))}
            </ul>
            <h4>Exposures</h4>
            <ul className="p-list">
              {env.exposures.map((exp) => (
                <li key={exp.name} className="p-list__item">
                  <a href={exp.url}>{exp.name}</a> ({exp.as_type})
                </li>
              ))}
            </ul>
          </div>
          <div className="p-card__footer">
            <button className="p-button--positive" onClick={() => handleRelaunch(false)}>
              Relaunch
            </button>
            <button className="p-button" onClick={() => handleRelaunch(true)}>
              Relaunch + Reset data
            </button>
            <button className="p-button--negative" onClick={handleTeardown}>
              Tear down
            </button>
          </div>
        </div>
      ) : (
        <div className="p-card">
          <div className="p-card__content">
            <p>No environment running</p>
            <button className="p-button--positive" onClick={handleLaunch}>
              Launch environment
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
