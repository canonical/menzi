import { Button, Icon } from '@canonical/react-components';
import { useSession } from '../stores/useSession';
import { S } from '../strings/catalogue';

export function SignOutNavItem() {
  const { signOut } = useSession();

  return (
    <Button
      appearance="link"
      className="p-side-navigation__link app-nav-button"
      onClick={() => void signOut()}
    >
      <Icon className="p-side-navigation__icon" name="user" />
      <span className="p-side-navigation__label">{S.app.signOut}</span>
    </Button>
  );
}
