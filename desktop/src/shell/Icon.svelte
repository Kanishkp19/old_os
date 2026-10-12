<script>
  export let name;
  export let size = 22;
  const paths = {
    Home: 'M3 11 12 3l9 8M5 10v11h5v-7h4v7h5V10',
    Photos: 'M4 3h16v18H4zM4 16l5-5 4 4 3-3 4 4M15 7h.01',
    Files: 'M3 7V4h7l2 3h9v13H3zM3 10h18',
    Browser: 'M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0ZM3 12h18M12 3c5 5 5 13 0 18-5-5-5-13 0-18Z',
    YouTube: 'M3 6h18v12H3zM10 9l5 3-5 3z',
    Music: 'M9 17V5l11-2v12M9 8l11-2M9 17a3 3 0 1 1-3-3h3M20 15a3 3 0 1 1-3-3h3',
    Notes: 'M5 3h14v18H5zM8 8h8M8 12h8M8 16h5',
    Calculator: 'M5 2h14v20H5zM8 6h8M8 11h1M15 11h1M8 15h1M15 15h1M8 19h1M15 19h1',
    Devices: 'M2 4h14v12H2zM6 20h7M9 16v4M18 9h4v12h-4z',
    Settings: 'M4 7h16M4 17h16M8 4v6M16 14v6',
    Network: 'M2 8a16 16 0 0 1 20 0M5 12a11 11 0 0 1 14 0M8 16a6 6 0 0 1 8 0M12 20h.01',
    Storage: 'M4 4h16v16H4zM4 14h16M16 17h.01',
    Lock: 'M6 10h12v11H6zM8 10V6a4 4 0 0 1 8 0v4',
    Close: 'm6 6 12 12M6 18 18 6',
    Minimize: 'M5 12h14', Maximize: 'M5 5h14v14H5z',
    Restore: 'M8 5V3h13v13h-2M3 8h13v13H3z',
    Volume: 'M3 9h4l5-4v14l-5-4H3zM16 8a6 6 0 0 1 0 8M19 5a10 10 0 0 1 0 14',
    Sun: 'M16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0ZM12 2v2M12 20v2M2 12h2M20 12h2M5 5l1 1M18 18l1 1M5 19l1-1M18 6l1-1',
    Resize: 'm9 21 12-12M15 21l6-6',
  };
</script>
<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={paths[name] || paths.Settings}/></svg>
