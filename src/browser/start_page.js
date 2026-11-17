(() => {
  const clock = document.getElementById('clock');
  const date = document.getElementById('date');
  function update() {
    if (document.hidden) return;
    const now = new Date();
    clock.textContent = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' }).format(now);
    date.textContent = new Intl.DateTimeFormat('pt-BR', { weekday: 'long', day: 'numeric', month: 'long' }).format(now);
    document.getElementById('greeting').textContent = now.getHours() < 12 ? 'BOM DIA' : now.getHours() < 18 ? 'BOA TARDE' : 'BOA NOITE';
  }
  update();
  document.querySelector('.manage-shortcuts').addEventListener('click', () => {
    window.webkit.messageHandlers.kairoBookmarks.postMessage('manage');
  });
  document.addEventListener('visibilitychange', update);
  setInterval(update, 15000);
})();
