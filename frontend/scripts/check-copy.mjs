import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const cataloguePath = resolve('src/strings/catalogue.ts')
const source = readFileSync(cataloguePath, 'utf8')

const startMarker = 'export const S ='
const start = source.indexOf(startMarker)
if (start === -1) {
  console.error(`check-copy: could not find "${startMarker}" in ${cataloguePath}`)
  process.exit(1)
}

const body = source.slice(start + startMarker.length)

const stringPattern = /^\s*[A-Za-z0-9_]+:\s*'((?:[^'\\]|\\.)*)'\s*,?$/gm
const values = [...body.matchAll(stringPattern)].map((match) => match[1])

const forbidden = [
  { pattern: /\bplease\b/i, message: '"please" is not allowed (§13.4)' },
  { pattern: /\bthank (you|s)?\b/i, message: '"thank you" is not allowed (§13.4)' },
  { pattern: /\be\.g\.(?![a-z])/i, message: '"e.g." is not allowed (§13.4); use "for example"' },
  { pattern: /\bi\.e\.(?![a-z])/i, message: '"i.e." is not allowed (§13.4); use "that is"' },
  { pattern: /\bclick here\b/i, message: '"click here" is not allowed (§13.4)' },
]

const violations = []

for (const [index, value] of values.entries()) {
  for (const { pattern, message } of forbidden) {
    if (pattern.test(value)) {
      violations.push(`string #${index + 1} (${JSON.stringify(value)}): ${message}`)
    }
  }
  if (value !== value.trim()) {
    violations.push(`string #${index + 1} (${JSON.stringify(value)}): leading or trailing whitespace`)
  }
  if (value === '') {
    violations.push(`string #${index + 1}: empty string`)
  }
  if (/\s/.test(value) && /^[a-z]/.test(value)) {
    violations.push(
      `string #${index + 1} (${JSON.stringify(value)}): multi-word strings must start with a capital letter (sentence case, §13.4)`,
    )
  }
}

if (violations.length > 0) {
  console.error(`check-copy: ${violations.length} violation(s) in src/strings/catalogue.ts`)
  for (const violation of violations) {
    console.error(`  - ${violation}`)
  }
  process.exit(1)
}

console.log(`check-copy: ok (${values.length} strings)`)