import { useState } from 'react';
import { Link, useParams, type LinkProps } from 'react-router-dom';
import {
  AppAside,
  ApplicationLayout,
  Button,
  EmptyState,
  Icon,
  NotificationConsumer,
  SegmentedControl,
  SideNavigation,
  SidePanel,
  SkipLink,
  Tabs,
  ThemeSwitcher,
  type SideNavigationProps,
} from '@canonical/react-components';
import { useAuthStore } from '../../stores/auth';
import { Logo } from '../../components/Logo';
import { EnvPanel } from './EnvPanel';
import { TranscriptView } from './TranscriptView';
import { S } from '../../strings/catalogue';

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

const WORKSPACE_TABS = [
  { id: 'changes', label: 'Changes' },
  { id: 'env', label: S.env.title },
  { id: 'terminal', label: 'Terminal' },
  { id: 'preview', label: 'Preview' },
  { id: 'files', label: 'Files' },
  { id: 'design', label: 'Design' },
];

const TAB_PENDING: Record<string, { title: string; body: string }> = {
  changes: {
    title: 'Changes are not available yet',
    body: 'This tab needs a version control service that has not landed yet.',
  },
  terminal: {
    title: 'Terminals are not available yet',
    body: 'This tab needs a PTY service that has not landed yet.',
  },
  preview: {
    title: 'Previews are not available yet',
    body: 'This tab needs the exposure gateway and a permission service that have not landed yet.',
  },
  files: {
    title: 'Files are not available yet',
    body: 'This tab needs a file service that has not landed yet.',
  },
  design: {
    title: 'Design is not available yet',
    body: 'This tab needs the design service that has not landed yet.',
  },
};

type NavItems = NonNullable<SideNavigationProps<LinkProps>['items']>;

export function SessionPage() {
  const { projectId, sessionId } = useParams<{ projectId: string; sessionId: string }>();
  const [activeTab, setActiveTab] = useState('env');
  const { logout } = useAuthStore();

  const sessionKnown = UUID_PATTERN.test(sessionId ?? '');

  const threadItems: NavItems = [
    {
      items: [
        { icon: 'code', label: S.nav.projects, to: '/projects' },
        {
          icon: 'quote',
          label: sessionKnown ? S.session.title : S.session.threads.emptyTitle,
          to: '',
        },
      ],
    },
  ];

  return (
    <>
      <SkipLink mainId="session-main" />
      <ApplicationLayout<LinkProps>
        mainId="session-main"
        logo={<Logo />}
        sideNavigation={
          <SideNavigation<LinkProps> hasIcons items={threadItems} linkComponent={Link} />
        }
        aside={
          <AppAside>
            <SidePanel isOpen pinned width="wide">
              <SidePanel.Header>
                <SidePanel.HeaderTitle>
                  {projectId ? `${S.session.title}: ${projectId}` : S.session.title}
                </SidePanel.HeaderTitle>
                <SidePanel.HeaderControls>
                  <Button appearance="link" onClick={logout}>
                    {S.app.signOut}
                  </Button>
                </SidePanel.HeaderControls>
              </SidePanel.Header>
              <SidePanel.Content>
                <Tabs
                  links={WORKSPACE_TABS.map((tab) => ({
                    label: tab.label,
                    active: activeTab === tab.id,
                    onClick: () => setActiveTab(tab.id),
                  }))}
                />
                {activeTab === 'env' && sessionKnown && sessionId ? (
                  <EnvPanel sessionId={sessionId} />
                ) : (
                  <PendingTab tabId={activeTab} />
                )}
              </SidePanel.Content>
            </SidePanel>
          </AppAside>
        }
        status={
          <div className="u-flex u-align--center u-justify--end">
            <ThemeSwitcher />
            <SegmentedControl
              segments={[
                {
                  label: S.session.composer.queue,
                  content: S.session.composer.queue,
                },
                {
                  label: S.session.composer.steer,
                  content: S.session.composer.steer,
                },
              ]}
            />
          </div>
        }
      >
        <div className="app-content">
          {sessionKnown && sessionId ? (
            <TranscriptView sessionId={sessionId} />
          ) : (
            <EmptyState title={S.session.conversation.emptyTitle} image={<Icon name="quote" />}>
              <p>{S.session.conversation.emptyBody}</p>
            </EmptyState>
          )}
        </div>
        <NotificationConsumer />
      </ApplicationLayout>
    </>
  );
}

function PendingTab({ tabId }: { tabId: string }) {
  const pending = TAB_PENDING[tabId];
  if (!pending) return null;
  return (
    <EmptyState title={pending.title} image={<Icon name="help" />}>
      <p>{pending.body}</p>
    </EmptyState>
  );
}
