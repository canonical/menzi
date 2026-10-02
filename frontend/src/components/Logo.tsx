import canonicalLogo from '../assets/canonical-logo.png';

export function LogoIcon() {
  return (
    <img
      className="menzi-logo__icon"
      src={canonicalLogo}
      alt=""
      aria-hidden="true"
    />
  );
}

export function Logo() {
  return (
    <span className="menzi-logo">
      <LogoIcon />
      <h4 className="menzi-logo__name">Menzi</h4>
    </span>
  );
}
