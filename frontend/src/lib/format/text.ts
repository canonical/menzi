export function oneLine(value: string, limit: number): string {
  const squashed = value.replace(/\s+/g, ' ').trim();
  if (squashed.length <= limit) return squashed;
  return `${squashed.slice(0, limit).trimEnd()}...`;
}
