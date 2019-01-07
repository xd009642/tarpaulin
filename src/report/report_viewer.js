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

  let commonPath = [...files[0].path];
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

    return {
      is_folder: true,
      path: [folder],
      parent: files[0].parent,
      children: findFolders(filesInFolder), // recursion
      covered: filesInFolder.reduce((sum, file) => sum + file.covered, 0),
      coverable: filesInFolder.reduce((sum, file) => sum + file.coverable, 0),
    };
  });

  return [
    ...folders,
    ...files.filter(file => file.path.length === 1),
  ];
}

class App extends React.Component {
  constructor(...args) {
    super(...args);

    const commonPath = findCommonPath(data.files);
    let files = data.files.map(file => ({...file, path: file.path.slice(commonPath.length), parent: commonPath}));

    data = {is_folder: true, children: findFolders(files), path: commonPath, parent: []};

    this.state = {
      commonPath,
      current: [data],
    };
  }

  render() {
    const {current} = this.state;
    if (!current || !current.length) {
      return null;
    }
    const file = current[current.length - 1];

    let w = null;
    if (file.is_folder) {
      w = e(FilesList, {
        folder: file,
        onSelectFile: this.selectFile.bind(this),
        onBack: current.length > 1 ? this.back.bind(this) : null,
      });
    } else {
      w = e(DisplayFile, {
        file,
        onBack: this.back.bind(this),
      });
    }

    return e('div', {className: 'app'}, w);
  }

  selectFile(file) {
    this.setState(({current}) => {
      return {current: [...current, file]};
    });
  }

  back(file) {
    this.setState(({current}) => {
      return {current: current.slice(0, current.length - 1)};
    });
  }
}

function FilesList({folder, onSelectFile, onBack}) {
  let files = folder.children;
  return e('div', {className: 'display-folder'},
    e(FileHeader, {file: folder, onBack}),
    e('table', {className: 'files-list'},
      e('thead', {className: 'files-list__head'},
        e('tr', null,
          e('th', null, "Path"),
          e('th', null, "Coverage")
        )
      ),
      e('tbody', {className: 'files-list__body'},
        files.map(file => e(File, {file, onClick: onSelectFile}))
      )
    )
  );
}

function File({file, onClick}) {
  const coverage = file.coverable ? file.covered / file.coverable * 100 : -1;

  return e('tr', {
      className: 'files-list__file'
        + (coverage >= 0 && coverage < 50 ? ' files-list__file_low': '')
        + (coverage >= 50 && coverage < 80 ? ' files-list__file_medium': '')
        + (coverage >= 80 ? ' files-list__file_high': '')
        + (file.is_folder ? ' files-list__file_folder': ''),
      onClick: () => onClick(file),
    },
    e('td', null, pathToString(file.path)),
    e('td', null,
      file.covered + ' / ' + file.coverable +
      (coverage >= 0 ? ' (' + coverage.toFixed(2) + '%)' : '')
    )
  );
}

function DisplayFile({file, onBack}) {
  return e('div', {className: 'display-file'},
    e(FileHeader, {file, onBack}),
    e(FileContent, {file})
  );
}

function FileHeader({file, onBack}) {
  return e('div', {className: 'file-header'},
    onBack ? e('a', {className: 'file-header__back', onClick: onBack}, 'Back') : null,
    e('div', {className: 'file-header__name'}, pathToString([...file.parent, ...file.path])),
    e('div', {className: 'file-header__stat'},
      'Covered: ' + file.covered + ' of ' + file.coverable +
      (file.coverable ? ' (' + (file.covered / file.coverable * 100).toFixed(2) + '%)' : '')
    )
  );
}

function FileContent({file}) {
  return e('div', {className: 'file-content'},
    file.content.split(/\r?\n/).map((line, index) => {
      const trace = file.traces.find(trace => trace.line === index + 1);
      const covered = trace && trace.stats.Line;
      const uncovered = trace && !trace.stats.Line;
      return e('pre', {
          className: 'code-line'
            + (covered ? ' code-line_covered' : '')
            + (uncovered ? ' code-line_uncovered' : ''),
          title: trace ? JSON.stringify(trace.stats, null, 2) : null,
        }, line);
    })
  );
}

ReactDOM.render(e(App), document.getElementById('root'));
