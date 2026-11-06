const motionHost = window.__TAURI__.core;
const motionList = document.querySelector("#motion-assets");
const motionPlayer = document.querySelector("#motion-player");
const motionStatus = document.querySelector("#motion-status");
const motionOpen = document.querySelector("#motion-open");

async function loadMotionCompositions() {
  try {
    const result = await motionHost.invoke("execute_command", {
      id: "project.list_assets",
      params: {},
    });
    const assets = result?.assets ?? [];
    const compositions = assets.filter(asset =>
      asset.id?.startsWith("motion_") && asset.path?.toLowerCase().endsWith(".ecproj"));
    if (!compositions.length) {
      motionStatus.textContent = "Add an EffectCraft project with a motion_ asset ID to list it here.";
      return;
    }
    for (const composition of compositions) {
      const choice = document.createElement("button");
      choice.type = "button";
      choice.textContent = composition.name;
      choice.onclick = () => selectMotionComposition(assets, composition, choice);
      choice.setAttribute("aria-pressed", "false");
      motionList.append(choice);
    }
    motionList.firstElementChild.click();
  } catch (error) {
    motionStatus.textContent = String(error);
  }
}

function selectMotionComposition(assets, composition, choice) {
  for (const button of motionList.children) {
    button.setAttribute("aria-pressed", String(button === choice));
  }
  const preview = assets.find(asset => asset.id === `${composition.id}_preview`);
  motionPlayer.pause();
  motionPlayer.hidden = !preview?.path;
  if (preview?.path) {
    motionPlayer.src = motionHost.convertFileSrc(preview.path);
  } else {
    motionPlayer.removeAttribute("src");
    motionPlayer.load();
  }
  motionStatus.textContent = preview?.path ? composition.name : "This composition has no linked preview.";
  motionOpen.disabled = false;
  motionOpen.onclick = async () => {
    try {
      await motionHost.invoke("execute_command", {
        id: "workspace.open",
        params: { path: composition.path },
      });
    } catch (error) {
      motionStatus.textContent = String(error);
    }
  };
}

loadMotionCompositions();
