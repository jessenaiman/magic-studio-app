// A one-use navigation request; project data remains in the native documents.
let target: {project:string; time:number} | undefined;
export function requestAuthoringTime(project:string,time:number) { target={project,time}; }
export function takeAuthoringTime(project:string) {
  const current=target;
  target=undefined;
  return current?.project===project?current.time:undefined;
}
