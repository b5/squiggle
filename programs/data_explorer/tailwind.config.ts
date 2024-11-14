import type { Config } from 'tailwindcss'

const config: Config = {
	darkMode: ['class'],
	content: [
    './src/pages/**/*.{js,ts,jsx,tsx,mdx}',
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    './src/app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
  	extend: {
  		fontFamily: {
  			sans: ['Inter, sans-serif', { fontFeatureSettings: 'cv11"' }]
  		},
  		colors: {
  			n0pink: {
  				'100': '#FFF4F3',
  				'200': '#FFE9E6',
  				'300': '#FFD2CC',
  				'400': '#FFBCB3',
  				'500': '#FFAC9C',
  				'600': '#E2847D',
  				'700': '#CC6E66',
  				'800': '#AF584F',
  				'900': '#99463D'
  			},
  			irohGray: {
  				'50': '#FAFAFA',
  				'100': '#F8F8F8',
  				'200': '#E4E4E7',
  				'300': '#D4D4D8',
  				'400': '#A1A1AA',
  				'500': '#71717A',
  				'600': '#52525B',
  				'700': '#3B3B3B',
  				'800': '#27272A',
  				'900': '#18181B',
  				'1000': '#0E0E0F'
  			},
  			irohPurple: {
  				'50': '#EBEBFF',
  				'100': '#E1E1F9',
  				'200': '#C7C7F9',
  				'300': '#ADADF7',
  				'400': '#9494F7',
  				'500': '#7C7CFF',
  				'600': '#5454C6',
  				'700': '#4242A8',
  				'800': '#393999',
  				'900': '#2B2B7F',
  				'950': '#1B1B4C'
  			},
  			sidebar: {
  				DEFAULT: 'hsl(var(--sidebar-background))',
  				foreground: 'hsl(var(--sidebar-foreground))',
  				primary: 'hsl(var(--sidebar-primary))',
  				'primary-foreground': 'hsl(var(--sidebar-primary-foreground))',
  				accent: 'hsl(var(--sidebar-accent))',
  				'accent-foreground': 'hsl(var(--sidebar-accent-foreground))',
  				border: 'hsl(var(--sidebar-border))',
  				ring: 'hsl(var(--sidebar-ring))'
  			}
  		},
  		borderRadius: {
  			lg: 'var(--radius)',
  			md: 'calc(var(--radius) - 2px)',
  			sm: 'calc(var(--radius) - 4px)'
  		}
  	}
  },
  // plugins: [require("tailwindcss-animate")],
}
export default config
