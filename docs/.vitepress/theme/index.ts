import DefaultTheme from "vitepress/theme";
import type { Theme } from "vitepress";
import HomeExperience from "./components/HomeExperience.vue";
import PackageLab from "./components/PackageLab.vue";
import HomeExperienceZh from "./components/HomeExperienceZh.vue";
import HomeExperienceJa from "./components/HomeExperienceJa.vue";
import "@fontsource/klee-one/400.css";
import "@fontsource/klee-one/600.css";
import { installQueryLanguage } from "./query-language";
import { installApiReferenceNavigation } from "./api-reference-navigation";
import "./style.css";

const theme: Theme = {
  extends: DefaultTheme,
  enhanceApp({ app, router }) {
    app.component("HomeExperience", HomeExperience);
    app.component("PackageLab", PackageLab);
    app.component("HomeExperienceZh", HomeExperienceZh);
    app.component("HomeExperienceJa", HomeExperienceJa);

    if (typeof window === "undefined") return;

    installApiReferenceNavigation();
    installQueryLanguage(router);
  }
};

export default theme;
