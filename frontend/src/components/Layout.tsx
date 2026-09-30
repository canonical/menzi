import { ReactNode, useEffect } from 'react';
import { Link, useLocation, type LinkProps } from 'react-router-dom';
import {
  ApplicationLayout,
  Button,
  NotificationConsumer,
  SideNavigation,
  SkipLink,
  type SideNavigationProps,
} from '@canonical/react-components';
import { useQuery } from '@tanstack/react-query';
import { Logo } from './Logo';
import { ProjectSelector } from './ProjectSelector';
import { ThemeNavItem } from './ThemeNavItem';
import { listProjects } from '../lib/api/projects';
import { queryKeys, routes, type ProjectSection } from '../lib/routes';
import { useActiveProject } from '../stores/activeProject';
import { useAuthStore } from '../stores/auth';
import { useSession } from '../stores/useSession';
import { S } from '../strings/catalogue';

interface LayoutProps {
  children: ReactNode;
}

type NavItems = NonNullable<SideNavigationProps<LinkProps>['items']>;

function sectionForPath(pathname: string): ProjectSection {
  if (pathname.endsWith('/design')) return 'design';
  if (pathname.endsWith('/project') || pathname.endsWith('/previews')) return 'project';
  return 'code';
}

export function Layout({ children }: LayoutProps) {
  const { user } = useAuthStore();
  const { signOut } = useSession();
  const location = useLocation();
  const { projectId, section, recordRoute } = useActiveProject();
  const segment = sectionForPath(location.pathname);

  useEffect(() => {
    if (projectId && (section !== segment || !projectId)) {
      recordRoute(projectId, segment);
    }
  }, [projectId, section, segment, recordRoute]);

  const projectsQuery = useQuery({
    queryKey: queryKeys.projects.all(),
    queryFn: () => listProjects(),
  });

  const projectName =
    (projectsQuery.data ?? []).find((project) => project.id === projectId)?.name ?? null;

  const projectPath = projectId
    ? {
        code: routes.projects.code(projectId),
        design: routes.projects.design(projectId),
        project: routes.projects.project(projectId),
      }
    : null;

  const navItems: NavItems = [
    {
      items: [
        { icon: 'code', label: S.nav.projects, to: routes.projects.list() },
        { icon: 'information', label: S.nav.needsYou, to: routes.inbox() },
        { icon: 'user', label: S.nav.settings, to: routes.settings() },
      ],
    },
    {
      items: [<ThemeNavItem key="theme" />],
    },
  ];

  if (projectPath) {
    navItems.unshift({
      headers: projectName ?? S.sections.project,
      items: [
        { icon: 'code', label: S.sections.code, to: projectPath.code },
        { icon: 'file-blank', label: S.sections.design, to: projectPath.design },
        { icon: 'settings', label: S.sections.project, to: projectPath.project },
      ],
    });
  }

  return (
    <>
      <SkipLink mainId="main-content" />
      <ApplicationLayout<LinkProps>
        mainId="main-content"
        logo={<Logo />}
        navItems={undefined}
        navLinkComponent={Link}
        // The navigation is always dark, whatever the page theme is. The
        // `--dark` marker is how Vanilla themes the icons that predate its
        // theme tokens, so without it they render dark on the dark panel.
        navigationClassName="app-navigation--dark"
        sideNavigation={
          <div>
            <div className="app-navigation-selector">
              <ProjectSelector sectionFor={sectionForPath} />
            </div>
            <SideNavigation<LinkProps> hasIcons items={navItems} linkComponent={Link} />
          </div>
        }
        status={
          <div className="app-status-bar">
            {user ? (
              <>
                <span className="u-off-screen">{user.name}</span>
                <Button appearance="link" onClick={() => void signOut()}>
                  {S.app.signOut}
                </Button>
              </>
            ) : null}
          </div>
        }
      >
        <div className="app-content">{children}</div>
        <NotificationConsumer />
      </ApplicationLayout>
    </>
  );
}
