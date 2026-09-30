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
import { useSession } from '../stores/useSession';
import {
  currentTheme,
  nextTheme,
  setTheme,
  themeGlyph,
  themeLabel,
  type Theme,
} from '../lib/theme';
import { S } from '../strings/catalogue';

interface LayoutProps {
  children: ReactNode;
}

type NavItems = NonNullable<SideNavigationProps<LinkProps>['items']>;

interface RailItem {
  key: string;
  icon: string;
  glyph?: string;
  label: string;
  to?: string;
  onClick?: () => void;
}

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
  // The navigation starts open on every page load. Collapsing it is a way of
  // getting the chat more room, not a preference worth carrying between visits,
  // so the choice is deliberately not persisted.
  const [navCollapsed, setNavCollapsed] = useState(false);
  const [theme, setThemeState] = useState<Theme>(currentTheme);

  const cycleTheme = useCallback(() => {
    const next = nextTheme(theme);
    setTheme(next);
    setThemeState(next);
  }, [theme]);

  const toggleNav = useCallback((collapsed: boolean) => {
    setNavCollapsed(collapsed);
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

  const railItems: RailItem[] = [
    ...(projectPath
      ? [
          { key: 'p-code', icon: 'code', label: S.sections.code, to: projectPath.code },
          { key: 'p-design', icon: 'file-blank', label: S.sections.design, to: projectPath.design },
          { key: 'p-project', icon: 'settings', label: S.sections.project, to: projectPath.project },
        ]
      : []),
    { key: 'projects', icon: 'code', label: S.nav.projects, to: routes.projects.list() },
    { key: 'inbox', icon: 'information', label: S.nav.needsYou, to: routes.inbox() },
    { key: 'settings', icon: 'user', label: S.nav.settings, to: routes.settings() },
    { key: 'theme', icon: 'glyph', glyph: themeGlyph(theme), label: themeLabel(theme), onClick: cycleTheme },
    ...(user
      ? [{ key: 'sign-out', icon: 'external-link', label: S.app.signOut, onClick: () => void signOut() }]
      : []),
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
    <div>
      <div className="app-navigation-selector">
        <ProjectSelector sectionFor={sectionForPath} />
        <Button
          appearance="base"
          className="app-navigation-hide"
          aria-label={S.nav.hideNavigation}
          aria-expanded={true}
          onClick={() => toggleNav(true)}
        >
          <Icon name="collapse" />
        </Button>
      </div>
      <SideNavigation<LinkProps> hasIcons items={navItems} linkComponent={Link} />
    </div>
  );

  const rail = (
    <nav className="app-rail is-dark" aria-label={S.nav.railLabel} data-testid="app-rail">
      <Button
        appearance="link"
        className="app-rail__item"
        aria-label={S.nav.showNavigation}
        aria-expanded={false}
        onClick={() => toggleNav(false)}
      >
        <Icon name="expand" />
      </Button>
      <ul className="app-rail__list">
        {railItems.map((item) => (
          <li key={item.key}>
            {item.to ? (
              <Link className="app-rail__item" to={item.to} title={item.label}>
                <Icon name={item.icon} />
                <span className="u-off-screen">{item.label}</span>
              </Link>
            ) : (
              <Button
                appearance="link"
                className="app-rail__item"
                title={item.label}
                onClick={item.onClick}
              >
                {item.glyph ? (
                  <span className="app-rail__glyph" aria-hidden="true">
                    {item.glyph}
                  </span>
                ) : (
                  <Icon name={item.icon} />
                )}
                <span className="u-off-screen">{item.label}</span>
              </Button>
            )}
          </li>
        ))}
      </ul>
    </nav>
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
        navigationClassName="app-navigation--dark"
        sideNavigation={navCollapsed ? undefined : navigation}
      >
        <div className={`app-content${navCollapsed ? ' app-content--rail' : ''}`}>
          {children}
        </div>
        <NotificationConsumer />
      </ApplicationLayout>
      {navCollapsed ? rail : null}
    </>
  );
}