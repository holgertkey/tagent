// A section title in the sidebar (a chapter without a page of its own) folds and unfolds
// on a click, not only on its chevron.
document.addEventListener('click', (ev) => {
    const title = ev.target.closest('.chapter-link-wrapper > span');
    if (title) {
        title.parentElement.parentElement.classList.toggle('expanded');
    }
});
