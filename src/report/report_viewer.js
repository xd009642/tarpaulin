const e = React.createElement;

function pathToString(path) {
  if (path[0] === '/') {
    return '/' + path.slice(1).join('/');
  } else {
    return path.join('/');
  }
}

function findCommonPath(files) {
  if (!files || !files.length) {
    return [];
  }

  function isPrefix(arr, prefix) {
    if (arr.length < prefix.length) {
      return false;
    }
    for (let i = prefix.length - 1; i >= 0; --i) {
      if (arr[i] !== prefix[i]) {
        return false;
      }
    }
    return true;
  }

  let commonPath = files[0].path.slice(0, -1);
  while (commonPath.length) {
    if (files.every(file => isPrefix(file.path, commonPath))) {
      break;
    }
    commonPath.pop();
  }
  return commonPath;
}

function findFolders(files) {
  if (!files || !files.length) {
    return [];
  }

  let folders = files.filter(file => file.path.length > 1).map(file => file.path[0]);
  folders = [...new Set(folders)]; // unique
  folders.sort();

  folders = folders.map(folder => {
    let filesInFolder = files
      .filter(file => file.path[0] === folder)
      .map(file => ({
        ...file,
        path: file.path.slice(1),
        parent: [...file.parent, file.path[0]],
      }));

    const children = findFolders(filesInFolder); // recursion

    return {
      is_folder: true,
      path: [folder],
      parent: files[0].parent,
      children,
      covered: children.reduce((sum, file) => sum + file.covered, 0),
      coverable: children.reduce((sum, file) => sum + file.coverable, 0),
      prevRun: children.some(file => file.prevRun) ? {
        covered: children.reduce((sum, file) => sum + (file.prevRun ? file.prevRun.covered : 0), 0),
        coverable: children.reduce((sum, file) => sum + (file.prevRun ? file.prevRun.coverable : 0), 0),
      } : null,
    };
  });

  return [...folders, ...files.filter(file => file.path.length === 1)];
}

class App extends React.Component {
  constructor(...args) {
    super(...args);

    this.state = {
      current: [],
      theme: document.documentElement.getAttribute('data-theme') || 'light',
    };
    this.updateStateFromLocation = this.updateStateFromLocation.bind(this);
    this.toggleTheme = this.toggleTheme.bind(this);
  }

  componentDidMount() {
    this.updateStateFromLocation();
    window.addEventListener('hashchange', this.updateStateFromLocation);
  }

  componentWillUnmount() {
    window.removeEventListener('hashchange', this.updateStateFromLocation);
  }

  updateStateFromLocation() {
    if (window.location.hash.length > 1) {
      const current = window.location.hash.slice(1).split('/').map(decodeURIComponent);
      this.setState({current});
    } else {
      this.setState({current: []});
    }
  }

  getCurrentPath() {
    let file = this.props.root;
    let path = [file];
    for (let p of this.state.current) {
      file = file.children.find(file => file.path[0] === p);
      if (!file) {
        return path;
      }
      path.push(file);
    }
    return path;
  }

  render() {
    const path = this.getCurrentPath();
    const file = path[path.length - 1];

    let w = null;
    if (file.is_folder) {
      w = e(FilesList, {
        folder: file,
        onSelectFile: this.selectFile.bind(this),
        onBack: path.length > 1 ? this.back.bind(this) : null,
        rootPath: this.props.root.path,
        theme: this.state.theme,
        onToggleTheme: this.toggleTheme,
      });
    } else {
      w = e(DisplayFile, {
        file,
        onBack: this.back.bind(this),
        rootPath: this.props.root.path,
        theme: this.state.theme,
        onToggleTheme: this.toggleTheme,
      });
    }

    return e('div', {className: 'app'}, w);
  }

  selectFile(file) {
    this.setState(
      ({current}) => {
        return {current: [...current, file.path[0]]};
      },
      () => this.updateHash(),
    );
  }

  back(file) {
    this.setState(
      ({current}) => {
        return {current: current.slice(0, current.length - 1)};
      },
      () => this.updateHash(),
    );
  }

  updateHash() {
    if (!this.state.current || !this.state.current.length) {
      window.location = '#';
    } else {
      window.location = '#' + this.state.current.map(encodeURIComponent).join('/');
    }
  }

  toggleTheme() {
    const theme = this.state.theme === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', theme);
    try {
      window.localStorage.setItem('tarpaulin-theme', theme);
    } catch (_) {
      // Local files can be opened with storage disabled; the button still works.
    }
    this.setState({theme});
  }
}

function FilesList({folder, onSelectFile, onBack, rootPath, theme, onToggleTheme}) {
  let files = folder.children;
  return e(
    'div',
    {className: 'display-folder'},
    e(FileHeader, {file: folder, onBack, rootPath, theme, onToggleTheme}),
    e(
      'table',
      {className: 'files-list'},
      e('thead', {className: 'files-list__head'}, e('tr', null, e('th', null, 'Path'), e('th', null, 'Coverage'))),
      e(
        'tbody',
        {className: 'files-list__body'},
        files.map(file => e(File, {key: file.path[0], file, onClick: onSelectFile})),
      ),
    ),
  );
}

function File({file, onClick}) {
  const coverage = file.coverable ? (file.covered / file.coverable) * 100 : -1;
  const coverageDelta = file.prevRun && file.coverable && file.prevRun.coverable
    ? coverage - (file.prevRun.covered / file.prevRun.coverable) * 100
    : null;

  return e(
    'tr',
    {
      className:
        'files-list__file' +
        (coverage >= 0 && coverage < 50 ? ' files-list__file_low' : '') +
        (coverage >= 50 && coverage < 80 ? ' files-list__file_medium' : '') +
        (coverage >= 80 ? ' files-list__file_high' : '') +
        (file.is_folder ? ' files-list__file_folder' : ''),
    },
    e('td', null, e('button', {className: 'file-link', type: 'button', onClick: () => onClick(file)}, pathToString(file.path))),
    e(
      'td',
      null,
      file.covered + ' / ' + file.coverable + (coverage >= 0 ? ' (' + coverage.toFixed(2) + '%)' : ''),
      e(
        'span',
        {title: 'Change from the previous run'},
        coverageDelta !== null && coverageDelta !== 0 ? ` (${coverageDelta > 0 ? '+' : ''}${coverageDelta.toFixed(2)}%)` : '',
      ),
    ),
  );
}

function DisplayFile({file, onBack, rootPath, theme, onToggleTheme}) {
  return e('div', {className: 'display-file'}, e(FileHeader, {file, onBack, rootPath, theme, onToggleTheme}), e(FileContent, {file}));
}

function FileHeader({file, onBack, rootPath, theme, onToggleTheme}) {
  const coverage = file.coverable ? (file.covered / file.coverable) * 100 : null;
  const coverageDelta = file.prevRun && file.coverable && file.prevRun.coverable
    ? coverage - (file.prevRun.covered / file.prevRun.coverable) * 100
    : null;
  const fullPath = [...file.parent, ...file.path];
  const displayPath = pathToString(fullPath.slice(rootPath.length)) || 'Coverage report';

  return e(
    'div',
    {className: 'file-header'},
    onBack ? e('button', {className: 'file-header__back', type: 'button', onClick: onBack}, '← Back') : null,
    e('h1', {className: 'file-header__name', title: pathToString(fullPath)}, displayPath),
    e(
      'div',
      {className: 'file-header__stat'},
      file.covered + ' / ' + file.coverable + ' lines' + (coverage !== null ? ' (' + coverage.toFixed(2) + '%)' : ''),
      e(
        'span',
        {title: 'Change from the previous run'},
        coverageDelta !== null && coverageDelta !== 0 ? ` (${coverageDelta > 0 ? '+' : ''}${coverageDelta.toFixed(2)}%)` : '',
      ),
    ),
    e('button', {
      className: 'theme-toggle',
      type: 'button',
      onClick: onToggleTheme,
      'aria-label': theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode',
      'aria-pressed': theme === 'dark',
      title: theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode',
    }, theme === 'dark' ? '☀' : '☾'),
  );
}

function FileContent({file}) {
  return e(
    'pre',
    {className: 'file-content'},
    file.content.replace(/\r?\n$/, '').split(/\r?\n/).map((line, index) => {
      const trace = file.traces.find(trace => trace.line === index + 1);
      const hits = trace && trace.stats.Line;
      const covered = hits > 0;
      const uncovered = hits === 0;
      return e(
        'div',
        {className: 'code-text-container' + (covered ? ' line-covered' : '') + (uncovered ? ' line-uncovered' : ''), key: index},
        e('span', {className: 'line-number', 'aria-hidden': true}, index + 1),
        e(
          'code',
          {
            className: 'code-line' + (covered ? ' code-line_covered' : '') + (uncovered ? ' code-line_uncovered' : ''),
          },
          line
        ),
        e('span', {className: 'cover-indicator', title: covered ? `${hits} hits` : uncovered ? 'Not covered' : 'Not coverable'}, covered ? hits : uncovered ? '×' : ''),
      );
    }),
  );
}

(function () {
  const commonPath = findCommonPath(data.files);
  const prevFilesMap = new Map();

  previousData &&
    previousData.files.forEach(file => {
      const path = file.path.slice(commonPath.length).join('/');
      prevFilesMap.set(path, file);
    });

  const files = data.files.map(file => {
    const path = file.path.slice(commonPath.length);
    return {
      ...file,
      path,
      parent: commonPath,
      prevRun: prevFilesMap.get(path.join('/')) || null,
    };
  });

  const children = findFolders(files);

  const root = {
    is_folder: true,
    children,
    path: commonPath,
    parent: [],
    covered: children.reduce((sum, file) => sum + file.covered, 0),
    coverable: children.reduce((sum, file) => sum + file.coverable, 0),
    prevRun: previousData ? {
      covered: children.reduce((sum, file) => sum + (file.prevRun ? file.prevRun.covered : 0), 0),
      coverable: children.reduce((sum, file) => sum + (file.prevRun ? file.prevRun.coverable : 0), 0),
    } : null,
  };

  let theme;
  try {
    theme = window.localStorage.getItem('tarpaulin-theme');
  } catch (_) {}
  if (theme !== 'light' && theme !== 'dark') {
    theme = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  document.documentElement.setAttribute('data-theme', theme);

  ReactDOM.render(e(App, {root, prevFilesMap}), document.getElementById('root'));
})();
