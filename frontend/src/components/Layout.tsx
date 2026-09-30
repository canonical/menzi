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
import { SignOutNavItem } from './SignOutNavItem';
import { ThemeNavItem } from './ThemeNavItem';
import { listProjects } from '../lib/api/projects';
import { queryKeys, routes, type ProjectSection } from '../lib/routes';
import { useActiveProject } from '../stores/activeProject';
import { useAuthStore } from '../stores/auth';
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
  const location = useLocation();
  const { projectId, section, recordRoute } = useActiveProject();
  const segment = sectionForPath(location.pathname);
  // The navigation starts open on every page load. Collapsing it is a way of
  // getting the chat more room, not a preference worth carrying between visits,
  // so the choice is deliberately not persisted.
  const [navCollapsed, setNavCollapsed] = useState(false);
  const [drawerOpen, setDrawerOpen] = useState(false);

  const toggleNav = useCallback((collapsed: boolean) => {
    setNavCollapsed(collapsed);
    setDrawerOpen(false);
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
      items: [
        <ThemeNavItem key="theme" />,
        ...(user ? [<SignOutNavItem key="sign-out" />] : []),
      ],
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
  const navigation = (showCollapse: boolean) => (
    <div>
      <div className="app-navigation-selector">
        <ProjectSelector sectionFor={sectionForPath} />
        {showCollapse ? (
          <Button
            appearance="base"
            className="app-navigation-hide"
            aria-label={S.nav.hideNavigation}
            aria-expanded={true}
            onClick={() => toggleNav(true)}
          >
            <Icon name="collapse" />
          </Button>
        ) : null}
      </div>
      <SideNavigation<LinkProps> hasIcons items={navItems} linkComponent={Link} />
    </div>
  );



  return (
    <>
      <SkipLink mainId="main-content" />
      {navCollapsed ? (
        <div className="app-collapsed-bar is-dark">
          <Button
            appearance="base"
            className="app-collapsed-bar__toggle"
            aria-label={S.nav.showNavigation}
            aria-expanded={drawerOpen}
            aria-controls="app-navigation-drawer"
            onClick={() => setDrawerOpen((open) => !open)}
          >
            <Icon name="menu" />
          </Button>
          <Logo />
        </div>
      ) : null}
      <ApplicationLayout<LinkProps>
        mainId="main-content"
        logo={<Logo />}
        navItems={undefined}
        navLinkComponent={Link}
        // The navigation is always dark, whatever the page theme is. The
        // `--dark` marker is how Vanilla themes the icons that predate its
        // theme tokens, so without it they render dark on the dark panel.
        navigationClassName="app-navigation--dark"
        sideNavigation={navCollapsed ? undefined : navigation(true)}
      >
        <div className="app-content">{children}</div>
        <NotificationConsumer />
      </ApplicationLayout>
      {navCollapsed ? (
        <div
          className={`app-drawer is-dark${drawerOpen ? ' app-drawer--open' : ''}`}
          id="app-navigation-drawer"
          data-testid="app-drawer"
        >
          {navigation(false)}
        </div>
      ) : null}
    </>
  );
}