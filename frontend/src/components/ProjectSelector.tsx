import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Button, Input, Spinner } from '@canonical/react-components';
import { listProjects } from '../lib/api/projects';
import { projectSectionPath, queryKeys, type ProjectSection } from '../lib/routes';
import { useActiveProject } from '../stores/activeProject';
import { S } from '../strings/catalogue';

interface ProjectSelectorProps {
  sectionFor: (projectId: string) => ProjectSection;
}

export function ProjectSelector({ sectionFor }: ProjectSelectorProps) {
  const navigate = useNavigate();
  const { projectId, setProject } = useActiveProject();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const rootRef = useRef<HTMLDivElement | null>(null);

  const projectsQuery = useQuery({
    queryKey: queryKeys.projects.all(),
    queryFn: () => listProjects(),
  });

  const projects = useMemo(() => projectsQuery.data ?? [], [projectsQuery.data]);
  const selectedProject = projects.find((project) => project.id === projectId) ?? null;
  const filtered = useMemo(() => {
    const term = search.trim().toLowerCase();
    if (!term) return projects;
    return projects.filter((project) =>
      `${project.name} ${project.slug} ${project.description ?? ''}`.toLowerCase().includes(term),
    );
  }, [projects, search]);

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target || rootRef.current?.contains(target)) return;
      setOpen(false);
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, [open]);

  if (projectsQuery.isLoading) {
    return (
      <div className="u-padding--bottom">
        <Spinner text={S.projects.selector.loading} />
      </div>
    );
  }

  if (projectsQuery.error) {
    return (
      <p className="p-text--default" data-testid="project-selector-error">
        {S.projects.selector.unavailable}
      </p>
    );
  }

  if (projects.length === 0) {
    return (
      <p data-testid="project-selector-empty">
        {S.projects.selector.empty}
      </p>
    );
  }

  return (
    <div ref={rootRef} className="project-selector-section">
      <span className="project-selector__label">{S.projects.selector.label}</span>
      <Button
        appearance="base"
        className="project-selector__toggle"
        type="button"
        aria-expanded={open}
        aria-haspopup="listbox"
        onClick={() => {
          setOpen((value) => !value);
          setSearch('');
        }}
      >
        <svg className="project-selector__folder-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" /></svg>
        <span className="project-selector__name">{selectedProject?.name ?? 'Select project'}</span>
        <svg className="project-selector__chevron" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><polyline points="4 6 8 10 12 6" /></svg>
      </Button>
      {open ? (
        <div className="project-selector__dropdown">
          <Input
            type="search"
            className="project-selector__search-input"
            label={S.projects.selector.searchPlaceholder}
            labelClassName="u-off-screen"
            placeholder={`${S.projects.selector.searchPlaceholder}…`}
            autoFocus
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Escape') setOpen(false);
            }}
          />
          <div className="project-selector__list" role="listbox" aria-label={S.projects.selector.label}>
            {filtered.map((project) => (
              <Button
                appearance="base"
                key={project.id}
                type="button"
                className={`project-selector__item${project.id === projectId ? ' is-selected' : ''}`}
                role="option"
                aria-selected={project.id === projectId}
                onClick={() => {
                  setProject(project.id);
                  setOpen(false);
                  navigate(projectSectionPath(project.id, sectionFor(project.id)));
                }}
              >
                <span className="project-item__name">{project.name}</span>
                <span className="project-item__slug">{project.slug}</span>
              </Button>
            ))}
            {filtered.length === 0 ? <p className="project-selector__empty">No projects found.</p> : null}
          </div>
        </div>
      ) : null}
    </div>
  );
}
