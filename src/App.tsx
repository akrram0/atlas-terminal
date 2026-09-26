import { TitleBar } from './components/TitleBar';
import { Terminal } from './components/Terminal';

function App() {
  return (
    <div className="terminal-window">
      <TitleBar />
      <div className="terminal-container">
        <Terminal />
      </div>
    </div>
  );
}

export default App;
