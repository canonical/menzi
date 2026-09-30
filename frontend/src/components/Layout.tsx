import { ReactNode, useCallback, useEffect, useState } from 'react';
import { Link, useLocation, type LinkProps } from 'react-router-dom';
import {
  ApplicationLayout,
  Button,
  Icon,
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

const NAV_COLLAPSED_KEY = 'menzi.nav.collapsed';

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(NAV_COLLAPSED_KEY) === '1';
  } catch {
    return false;
  }
}

export function Layout({ children }: LayoutProps) {
  const { user } = useAuthStore();
  const { signOut } = useSession();
  const location = useLocation();
  const { projectId, section, recordRoute } = useActiveProject();
  const segment = sectionForPath(location.pathname);
  const [navCollapsed, setNavCollapsed] = useState(readCollapsed);

  const toggleNav = useCallback((collapsed: boolean) => {
    setNavCollapsed(collapsed);
    try {
      localStorage.setItem(NAV_COLLAPSED_KEY, collapsed ? '1' : '0');
    } catch {
      return;
    }
  }, []);

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
          navCollapsed ? undefined : (
            <div>
              <div className="app-navigation-selector">
                <ProjectSelector sectionFor={sectionForPath} />
                <Button
                  appearance="base"
                  className="app-navigation-hide"
                  aria-label={S.nav.hideNavigation}
                  onClick={() => toggleNav(true)}
                >
                  <Icon name="collapse" />
                </Button>
              </div>
              <SideNavigation<LinkProps> hasIcons items={navItems} linkComponent={Link} />
            </div>
          )
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
        {navCollapsed ? (
          <Button
            className="app-navigation-show"
            aria-label={S.nav.showNavigation}
            aria-expanded={false}
            onClick={() => toggleNav(false)}
          >
            <Icon name="expand" />
            {S.nav.showNavigation}
          </Button>
        ) : null}
        <NotificationConsumer />
      </ApplicationLayout>
    </>
  );
}
