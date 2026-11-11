const parloIcon = new URL("./assets/parlo-assistant-icon.png", import.meta.url).href;
const parloIcon2x = new URL("./assets/parlo-assistant-icon@2x.png", import.meta.url).href;
export default function AssistantIdentity({ streaming = false }: { streaming?: boolean }) {
  return <div className="message-author"><img className="assistant-avatar" src={parloIcon} srcSet={`${parloIcon2x} 2x`} width={22} height={22} alt="" /><span>PARLO</span>{streaming && <i className="pulse-dot"/>}</div>;
}
