export function LogoIcon() {
  return (
    <svg
      className="menzi-logo__icon"
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      width="24"
      height="24"
      role="img"
      aria-label="Menzi"
    >
      <path
        fill="currentColor"
        d="M4 6v12h2.8v-6.6L12 16.2l5.2-4.8V18H20V6h-3.8L12 10.1 7.8 6z"
      />
    </svg>
  );
}

export function Logo() {
  return (
    <span className="menzi-logo">
      <LogoIcon />
      <span className="menzi-logo__name">Menzi</span>
    </span>
  );
}
