import { useEffect, useState } from 'react';
import { listProjects } from './api';
import type { Project } from '../../lib/types';

export function ProjectsPage() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    listProjects()
      .then(setProjects)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  if (loading) {
    return <p>Loading projects...</p>;
  }

  return (
    <div>
      <h2 className="p-heading--3">Projects</h2>
      <div className="p-card">
        <div className="p-card__content">
          <table className="p-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Slug</th>
                <th>Role</th>
              </tr>
            </thead>
            <tbody>
              {projects.map((project) => (
                <tr key={project.id}>
                  <td><a href={`/projects/${project.id}`}>{project.name}</a></td>
                  <td>{project.slug}</td>
                  <td>{project.role}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
