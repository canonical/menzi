import { Button } from '@canonical/react-components';
import { useQuery } from '@tanstack/react-query';
import { getProviders, oidcStartUrl } from '../../lib/api/auth';
import { queryKeys } from '../../lib/routes';
import { S } from '../../strings/catalogue';

export function ProviderButtons({ redirectTo }: { redirectTo?: string }) {
  const providers = useQuery({
    queryKey: queryKeys.authProviders(),
    queryFn: getProviders,
    retry: false,
    staleTime: 5 * 60_000,
  });

  const list = providers.data?.oidc ?? [];
  if (list.length === 0) return null;

  return (
    <div className="app-auth__providers">
      {list.map((provider) => (
        <Button
          key={provider.id}
          className="app-auth__provider"
          appearance="base"
          as="a"
          href={oidcStartUrl(provider.id, redirectTo)}
          role="link"
        >
          {S.auth.continueWith.replace('{provider}', provider.label)}
        </Button>
      ))}
    </div>
  );
}
