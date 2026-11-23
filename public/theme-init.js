(function () {
  try {
    var saved = localStorage.getItem("antik:theme");
    var theme =
      saved || (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
    document.documentElement.dataset.theme = theme;
  } catch (error) {
    document.documentElement.dataset.theme = "light";
  }
})();
