import { useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { CustomSelect, Spinner } from '@canonical/react-components';
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

  const projectsQuery = useQuery({
    queryKey: queryKeys.projects.all(),
    queryFn: () => listProjects(),
  });

  const options = useMemo(
    () =>
      (projectsQuery.data ?? []).map((project) => ({
        value: project.id,
        label: project.name,
        selectedLabel: project.name,
        text: `${project.name} ${project.slug}`,
      })),
    [projectsQuery.data],
  );

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

  if (options.length === 0) {
    return (
      <p data-testid="project-selector-empty">
        {S.projects.selector.empty}
      </p>
    );
  }

  return (
    <div className="u-no-padding--bottom u-margin--bottom">
      <CustomSelect
        id="active-project"
        label={S.projects.selector.label}
        searchable="always"
        searchPlaceholder={S.projects.selector.searchPlaceholder}
        options={options}
        value={projectId}
        onChange={(value) => {
          setProject(value);
          navigate(projectSectionPath(value, sectionFor(value)));
        }}
        toggleClassName="u-no-margin--bottom"
        wrapperClassName="u-no-margin--bottom"
      />
    </div>
  );
}
