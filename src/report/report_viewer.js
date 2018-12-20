const e = React.createElement;

class App extends React.Component {
  constructor(...args) {
    super(...args);
    this.state = {
      data,
      currentFile: null,
    };
  }

  render() {
    const {data, currentFile} = this.state;
    return e('div', {className: 'app'},
      !currentFile ?
        e(FilesList, {
          files: data.files,
          onSelectFile: file => this.setState({currentFile: file}),
        })
      : e(DisplayFile, {
        file: currentFile,
        onBack: () => this.setState({currentFile: null}),
      })
    );
  }
}

function FilesList({files, onSelectFile}) {
  return e('table', {className: 'files-list'},
    e('thead', {className: 'files-list__head'},
      e('tr', null,
        e('th', null, "Path"),
        e('th', null, "Coverage")
      )
    ),
    e('tbody', {className: 'files-list__body'},
      files.map(file => e(File, {file, onClick: onSelectFile}))
    )
  );
}

function File({file, onClick}) {
  const coverage = file.coverable ? file.covered / file.coverable * 100 : -1;

  return e('tr', {
      className: 'files-list__file'
        + (coverage >= 0 && coverage < 50 ? ' files-list__file_low': '')
        + (coverage >= 50 && coverage < 80 ? ' files-list__file_medium': '')
        + (coverage >= 80 ? ' files-list__file_high': ''),
      onClick: () => onClick(file),
    },
    e('td', null, file.path),
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
    e('a', {className: 'file-header__back', onClick: onBack}, 'Back'),
    e('div', {className: 'file-header__name'}, file.path),
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
