// Aplica o tema antes do React montar, para o app não piscar no tema errado.
// Fica fora do bundle porque a CSP do Tauri bloqueia scripts inline.
// A lógica completa está em src/theme/theme.ts; a chave precisa ser a mesma.
(function () {
  var preference = "sistema";
  try {
    var stored = localStorage.getItem("easylobby.tema");
    if (stored === "claro" || stored === "escuro") preference = stored;
  } catch (e) {
    // Sem armazenamento, segue o sistema.
  }
  var dark = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
  document.documentElement.dataset.theme =
    preference === "sistema" ? (dark ? "escuro" : "claro") : preference;
})();
