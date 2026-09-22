import DefaultTheme from "vitepress/theme";
import type { Theme } from "vitepress";
import HomeExperience from "./components/HomeExperience.vue";
import PackageLab from "./components/PackageLab.vue";
import HomeExperienceZh from "./components/HomeExperienceZh.vue";
import { installQueryLanguage } from "./query-language";
import "./style.css";

const theme: Theme = {
  extends: DefaultTheme,
  enhanceApp({ app, router }) {
    app.component("HomeExperience", HomeExperience);
    app.component("PackageLab", PackageLab);
    app.component("HomeExperienceZh", HomeExperienceZh);

    if (typeof window === "undefined") return;

    installQueryLanguage(router);
  }
};

export default theme;
