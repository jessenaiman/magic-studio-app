// Shared report pieces for data views. Each owns its markup; global.css owns the
// look. Numbers stay tabular; a bar is a share of the column's largest value.
const reportNumber = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });
const reportCompact = new Intl.NumberFormat(undefined, { notation: 'compact', maximumFractionDigits: 1 });

function reportValue(value, { compact = false } = {}) {
  if (value === null || value === undefined || !Number.isFinite(Number(value))) return '—';
  return (compact ? reportCompact : reportNumber).format(Number(value));
}

// One headline number: label, value, what it means, and an optional signal or share.
class ReportStat {
  constructor({ label, value, detail = '', signal = null, share = null, title = null }) {
    this.element = el('div', { class: 'report', 'data-look': 'stat', title },
      el('span', { class: 'report-label' }, signal === null ? null : window.companionBoxes.dot(signal), label),
      el('strong', { class: 'report-value' }, value),
      share === null ? null : new ReportBar(share).element,
      detail ? el('span', { class: 'report-detail' }, detail) : null);
  }
}

// A thin proportional bar; share is 0..1. A native <progress>, so global.css styles
// it and assistive technology reads the value without any inline style.
class ReportBar {
  constructor(share) {
    this.element = el('progress', { class: 'report', 'data-look': 'bar', max: '1',
      value: String(Math.max(0, Math.min(1, Number(share) || 0))) });
  }
}

// A data table. A column is {label, value(row), numeric, bar, text(row)}; bar columns
// draw each value against the column's largest one.
class ReportTable {
  constructor({ columns, rows, empty = 'Nothing recorded.', caption = null }) {
    this.columns = columns;
    this.element = rows.length
      ? el('table', { class: 'report', 'data-look': 'table' },
        caption ? el('caption', {}, caption) : null,
        el('thead', {}, el('tr', {}, ...columns.map((column) => el('th', { 'data-numeric': String(!!column.numeric) }, column.label)))),
        el('tbody', {}, ...rows.map((row) => this.row(row, rows))))
      : el('p', { class: 'panel-empty', 'data-look': 'inline' }, empty);
  }
  row(row, rows) {
    return el('tr', {}, ...this.columns.map((column) => {
      const value = column.value(row);
      const text = column.text ? column.text(row) : column.numeric ? reportValue(value, column) : value;
      const max = column.bar ? Math.max(...rows.map((other) => Number(column.value(other)) || 0)) : 0;
      return el('td', { 'data-numeric': String(!!column.numeric), title: column.title?.(row) ?? null },
        column.bar ? new ReportBar(max ? Number(value) / max : 0).element : null, text);
    }));
  }
}

// A titled report block on the shared panel surface, with an optional footnote.
class ReportSection {
  constructor({ title, note = null, actions = null }, ...content) {
    const header = new PanelHeader({ id: null, title, actions });
    header.element.dataset.look = 'heading';
    header.element.classList.add('panel-head');
    this.element = el('section', { class: 'panel report-section', 'aria-label': title },
      header.element,
      el('div', { class: 'panel-body report-body' }, ...content),
      note ? el('p', { class: 'report-note' }, note) : null);
  }
}
