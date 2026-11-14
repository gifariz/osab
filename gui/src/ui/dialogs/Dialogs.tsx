import { useStore } from "../../state/store";
import { AreaAssignDialog, AreaLoadsDialog, AreaSectionsDialog, DivideAreasDialog } from "./AreaDialogs";
import { CombinationsDialog } from "./CombinationsDialog";
import { MoveDialog, ReplicateDialog } from "./EditDialogs";
import { ExamplesDialog, WelcomeDialog } from "./DemoDialogs";
import { FrameAssignDialog, FrameLoadsDialog, JointLoadsDialog, RestraintsDialog, SpringsDialog } from "./AssignDialogs";
import { MaterialsDialog, SectionsDialog } from "./DefineDialogs";
import { LoadCasesDialog, LoadPatternsDialog, ShowLoadsDialog } from "./LoadPatternDialogs";
import { AboutDialog, AnalysisOptionsDialog, CheckDialog, GridDialog, JointCoordsDialog, NewModelDialog, PreferencesDialog, RunCasesDialog } from "./ModelDialogs";
import { AngleLimitsDialog, AreaMeshDialog, FloorMeshingDialog, MeshSettingsDialog, SelectTypeDialog, SpecialJointDialog } from "./MeshDialogs";
import { DisplayOptionsDialog } from "./DisplayOptionsDialog";
import { AssignSpringDialog, SpringPropsDialog } from "./SpringDialogs";
import { AreaInsertionDialog, FrameInsertionDialog } from "./InsertionDialogs";
import { AreaModifiersDialog, FrameModifiersDialog } from "./ModifierDialogs";
import { AssignDiaphragmDialog, DiaphragmsDialog } from "./DiaphragmDialogs";
import { JointMassesDialog, MassSourceDialog, PDeltaDialog } from "./ModalDialogs";
import { OffsetsDialog } from "./OffsetsDialog";
import { ReleasesDialog } from "./ReleasesDialog";
import { SpectrumFunctionsDialog } from "./SpectrumDialogs";
import { StoryDataDialog } from "./StoryDialogs";
import { TablesDialog } from "./TablesDialog";
import { GroupsDialog } from "./GroupsDialog";
import { WallLabelsDialog } from "./WallLabelsDialog";

/** Shows the open dialog, if any. */
export function Dialogs() {
  const dialog = useStore((s) => s.dialog);
  switch (dialog) {
    case "new":
      return <NewModelDialog />;
    case "grid":
      return <GridDialog />;
    case "storyData":
      return <StoryDataDialog />;
    case "jointCoords":
      return <JointCoordsDialog />;
    case "materials":
      return <MaterialsDialog />;
    case "sections":
      return <SectionsDialog />;
    case "patterns":
      return <LoadPatternsDialog />;
    case "cases":
      return <LoadCasesDialog />;
    case "combos":
      return <CombinationsDialog />;
    case "restraints":
      return <RestraintsDialog />;
    case "springs":
      return <SpringsDialog />;
    case "jointLoads":
      return <JointLoadsDialog />;
    case "frameLoads":
      return <FrameLoadsDialog />;
    case "frameAssign":
      return <FrameAssignDialog />;
    case "frameModifiers":
      return <FrameModifiersDialog />;
    case "frameReleases":
      return <ReleasesDialog />;
    case "frameOffsets":
      return <OffsetsDialog />;
    case "frameInsertion":
      return <FrameInsertionDialog />;
    case "areaInsertion":
      return <AreaInsertionDialog />;
    case "diaphragms":
      return <DiaphragmsDialog />;
    case "assignDiaphragm":
      return <AssignDiaphragmDialog />;
    case "jointMasses":
      return <JointMassesDialog />;
    case "spectrumFunctions":
      return <SpectrumFunctionsDialog />;
    case "massSource":
      return <MassSourceDialog />;
    case "pDelta":
      return <PDeltaDialog />;
    case "areaModifiers":
      return <AreaModifiersDialog />;
    case "areaSections":
      return <AreaSectionsDialog />;
    case "areaAssign":
      return <AreaAssignDialog />;
    case "areaLoads":
      return <AreaLoadsDialog />;
    case "divideAreas":
      return <DivideAreasDialog />;
    case "tables":
      return <TablesDialog />;
    case "check":
      return <CheckDialog />;
    case "about":
      return <AboutDialog />;
    case "examples":
      return <ExamplesDialog />;
    case "welcome":
      return <WelcomeDialog />;
    case "preferences":
      return <PreferencesDialog />;
    case "analysisOptions":
      return <AnalysisOptionsDialog />;
    case "runCases":
      return <RunCasesDialog />;
    case "groups":
      return <GroupsDialog />;
    case "pierLabels":
      return <WallLabelsDialog kind="pier" />;
    case "spandrelLabels":
      return <WallLabelsDialog kind="spandrel" />;
    case "meshSettings":
      return <MeshSettingsDialog />;
    case "angleLimits":
      return <AngleLimitsDialog />;
    case "areaMesh":
      return <AreaMeshDialog />;
    case "floorMeshing":
      return <FloorMeshingDialog />;
    case "lineSpringProps":
      return <SpringPropsDialog kind="line" />;
    case "areaSpringProps":
      return <SpringPropsDialog kind="area" />;
    case "lineSprings":
      return <AssignSpringDialog kind="line" />;
    case "areaSprings":
      return <AssignSpringDialog kind="area" />;
    case "specialJoint":
      return <SpecialJointDialog />;
    case "move":
      return <MoveDialog />;
    case "replicate":
      return <ReplicateDialog />;
    case "selectType":
      return <SelectTypeDialog />;
    case "displayOptions":
      return <DisplayOptionsDialog />;
    case "showLoads":
      return <ShowLoadsDialog />;
    default:
      return null;
  }
}
