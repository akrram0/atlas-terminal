/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      fontFamily: {
        sans: ['Inter', '-apple-system', 'BlinkMacSystemFont', 'SF Pro Text', 'Segoe UI', 'sans-serif'],
        mono: ['JetBrains Mono Nerd Font', 'MesloLGS NF', 'SF Mono', 'Consolas', 'monospace'],
      },
      spacing: {
        'space-1': '4px',
        'space-2': '8px',
        'space-3': '16px',
        'space-4': '24px',
        'space-5': '32px',
        'space-6': '48px',
        'space-7': '64px',
      },
      borderRadius: {
        'sm-token': '6px',
        'md-token': '10px',
        'lg-token': '12px',
      },
      colors: {
        'surface': {
          base: 'rgba(18, 18, 18, 0.60)',
          titlebar: 'rgba(28, 28, 32, 0.45)',
        },
        'border-subtle': 'rgba(255, 255, 255, 0.10)',
        'border-inner': 'rgba(255, 255, 255, 0.08)',
        'text-primary': '#F5F5F7',
        'text-muted': '#86868B',
        'text-disabled': '#55555B',
        'hover-overlay': 'rgba(255, 255, 255, 0.08)',
        'active-overlay': 'rgba(255, 255, 255, 0.12)',
        'close-hover': '#FF5F56',
      },
    },
  },
  plugins: [],
};
