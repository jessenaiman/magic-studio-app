// The Resources page intentionally starts small for the first video.
// Knowledge comes from Rustdoc and source; this surface is for visual references.
class ResourcesView {
  constructor(root) { this.root = root; }
  render() {
    if (this.root.firstElementChild) return;
    this.root.append(el('div', {id:'resources-page'},
      el('div', {class:'home-hero'}, el('div', {},
        el('h1', {}, 'Resources'),
        el('p', {class:'muted'}, 'Visual references, reusable ideas, and examples for your videos.'))),
      el('div', {id:'resources-empty'},
        el('p', {}, 'Room for your next idea.'),
        el('p', {class:'muted'}, 'A place to collect useful references as you create your first video.'))));
  }
}
const resourcesView = new ResourcesView($('view-skills'));
function renderResources() { resourcesView.render(); }
