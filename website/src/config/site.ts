export const siteConfig = {
  name: "whambam.dev",
  description:
    "Fast HTTP(S) load testing with a live terminal UI and hey-compatible flags.",
  // Release used for direct download links; update after each release.
  release: "v1.1.0",
  repo: "cloudy-native/whambam.dev",
  // The YouTube explainer embedded on the home page.
  video: {
    id: "x9BF5ERe7Tw",
    title: "whambam explainer video: load testing you can steer",
  },
  navItems: [
    { label: "Home", href: "/" },
    { label: "Docs", href: "/docs" },
    { label: "Technology", href: "/technology" },
  ],
  links: {
    github: "https://github.com/cloudy-native/whambam.dev",
    linkedin: "https://www.linkedin.com/in/stephenharrison/",
    portfolio: "https://dinsor.org",
  },
} as const;
