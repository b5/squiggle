import { emit } from '@tauri-apps/api/event';

const down = (e: KeyboardEvent) => {
  if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
    e.preventDefault();
    console.log('show-ui');
    emit('show-ui', {})
  } else if (e.key === "Escape") {
    emit('dismiss-ui', {})
  }
}
document.addEventListener("keydown", down)

const mouse = (e: MouseEvent) => {
  e.preventDefault();
  console.log('show-ui');
  emit('show-ui', {})
}
document.addEventListener("mouseup", mouse)
document.body.addEventListener('click', mouse); 