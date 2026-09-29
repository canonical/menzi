import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'

const restrictedElements = ['button', 'table', 'thead', 'tbody', 'tr', 'th', 'td', 'dialog', 'select', 'input', 'label', 'form']

const restrictedSyntax = restrictedElements.map((element) => ({
  selector: `JSXOpeningElement[name.name="${element}"]`,
  message: `Use the @canonical/react-components component for <${element}> instead of raw markup.`,
}))

export default tseslint.config(
  { ignores: ['dist'] },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'react-refresh/only-export-components': [
        'warn',
        { allowConstantExport: true },
      ],
    },
  },
  {
    files: ['**/*.d.ts'],
    rules: {
      '@typescript-eslint/no-empty-object-type': 'off',
      '@typescript-eslint/no-unused-vars': 'off',
      '@typescript-eslint/no-explicit-any': 'off',
    },
  },
  {
    files: ['src/features/**/*.{ts,tsx}', 'src/components/**/*.{ts,tsx}'],
    rules: {
      'no-restricted-syntax': ['error', ...restrictedSyntax],
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['@mui/*', 'antd', 'antd/*', 'bootstrap', 'bootstrap/*', 'react-bootstrap', 'react-bootstrap/*', 'reactstrap', 'reactstrap/*', 'semantic-ui-react', 'semantic-ui-react/*'],
              message: 'Use @canonical/react-components instead.',
            },
          ],
        },
      ],
    },
  },
)