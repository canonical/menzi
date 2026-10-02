import { ReactNode, useEffect, useState } from 'react';
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
  const [navHoverReady, setNavHoverReady] = useState(true);

  const toggleNav = (collapsed: boolean) => {
    setNavCollapsed(collapsed);
    if (collapsed) {
      setNavHoverReady(false);
      return;
    }
    setNavHoverReady(true);
  };

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
        { icon: 'user', label: S.nav.settings, to: routes.settings() },
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
  const navigation = (
    <div
      className="app-navigation-menu"
      onMouseLeave={() => {
        if (navCollapsed) setNavHoverReady(true);
      }}
    >
      <Button
        appearance="base"
        className="app-navigation-hide"
        aria-label={navCollapsed ? S.nav.showNavigation : S.nav.hideNavigation}
        title={navCollapsed ? S.nav.showNavigation : S.nav.hideNavigation}
        aria-expanded={!navCollapsed}
        onClick={(event) => {
          const nextCollapsed = !navCollapsed;
          toggleNav(nextCollapsed);
          if (nextCollapsed) event.currentTarget.blur();
        }}
      >
        <Icon name="pin" className={`app-navigation-toggle-icon${navCollapsed ? ' is-unpinned' : ''}`} />
      </Button>
      <div className="app-navigation-selector">
        <ProjectSelector sectionFor={sectionForPath} />
      </div>
      <SideNavigation<LinkProps> hasIcons items={navItems} linkComponent={Link} />
      <div className="app-navigation-footer">
        <SideNavigation<LinkProps>
          hasIcons
          items={[{ items: [<ThemeNavItem key="theme" />, ...(user ? [<SignOutNavItem key="sign-out" />] : [])] }]}
          linkComponent={Link}
        />
      </div>
    </div>
  );

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
        navigationClassName={`app-navigation--dark${navCollapsed ? ' app-navigation--collapsed' : ''}${navHoverReady ? ' app-navigation--hover-ready' : ''}`}
        sideNavigation={navigation}
      >
        <div className="app-content">
          {children}
        </div>
        <NotificationConsumer />
      </ApplicationLayout>
    </>
  );
}
