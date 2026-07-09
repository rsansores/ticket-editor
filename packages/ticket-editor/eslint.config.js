import js from '@eslint/js'
import tseslint from 'typescript-eslint'
import pluginVue from 'eslint-plugin-vue'
import globals from 'globals'

export default tseslint.config(
  // Generated / build output — never lint.
  { ignores: ['dist/**', 'src/wasm/**', 'node_modules/**'] },

  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs['flat/recommended'],

  {
    files: ['**/*.{ts,vue}'],
    languageOptions: {
      globals: { ...globals.browser },
      parserOptions: {
        // Parse <script lang="ts"> in .vue files with the TS parser.
        parser: tseslint.parser,
        extraFileExtensions: ['.vue'],
      },
    },
    rules: {
      // Production hygiene.
      'no-console': 'error',
      'no-debugger': 'error',
      '@typescript-eslint/no-explicit-any': 'error',
      '@typescript-eslint/no-non-null-assertion': 'error',
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
      'eqeqeq': ['error', 'smart'],
      // Components are file-name/prefix distinct; multi-word rule is noise here.
      'vue/multi-word-component-names': 'off',
      // Purely-stylistic template rules — formatting is not this linter's job
      // (eslint-plugin-vue is deprecating these). Correctness rules stay on.
      'vue/max-attributes-per-line': 'off',
      'vue/singleline-html-element-content-newline': 'off',
      'vue/html-self-closing': 'off',
      'vue/html-indent': 'off',
      'vue/html-closing-bracket-newline': 'off',
      'vue/first-attribute-linebreak': 'off',
      'vue/attributes-order': 'off',
      'vue/multiline-html-element-content-newline': 'off',
      'vue/singleline-html-element-content-newline': 'off',
    },
  },

  // The demo is allowed to log.
  {
    files: ['demo/**'],
    rules: { 'no-console': 'off' },
  },
)
