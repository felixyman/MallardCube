// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	site: 'https://felixyman.github.io/MallardCube/',
	base: '/MallardCube/',
	integrations: [
		starlight({
			title: 'MallardCube',
			description: 'Excel and XMLA frontend for DuckDB',
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/felixyman/MallardCube' }],
			sidebar: [
				{
					label: 'Getting started',
					items: [
						{ label: 'Introduction', slug: 'index' },
						{ label: 'Installation', slug: 'installation' },
						{ label: 'Connect Excel', slug: 'connect-excel' },
					],
				},
				{
					label: 'Configuration',
					items: [
						{ label: 'Model reference', slug: 'model' },
						{ label: 'AutoModel', slug: 'automodel' },
						{ label: 'Security and roles', slug: 'security' },
					],
				},
				{
					label: 'Operations',
					items: [
						{ label: 'Aggregations', slug: 'aggregations' },
						{ label: 'Deployment', slug: 'deployment' },
					],
				},
				{
					label: 'Behaviour reference',
					items: [
						{ label: 'About the reference', slug: 'reference' },
						{ label: 'Request shape', slug: 'reference/request-shape' },
						{ label: 'Excel metadata', slug: 'reference/excel-metadata' },
						{ label: 'Date Filters', slug: 'reference/date-filters' },
						{ label: 'Cellset shape', slug: 'reference/cellset-shape' },
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'MDX support', slug: 'mdx-support' },
						{ label: 'Migrate from SSAS', slug: 'migration' },
					],
				},
			],
		}),
	],
});
