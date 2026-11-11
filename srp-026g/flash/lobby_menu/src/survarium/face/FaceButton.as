package survarium.face
{
   import flash.events.Event;
   import survarium.controls.PanelButton;
   import survarium.data.LocalizationData;

   // The Face button (face-choice.md, Lobby): the player panel's, left of the Lobby button and as
   // it is (PanelButton), "FACE" (st_srp_face). Shown when srp has said the game data lists more
   // than one face. Its click opens the faces' window (FaceWindow) as the Lobby button's opens the
   // lobby window: the status panel tells the lobby (StatusPanel.FACE_WINDOW).
   public class FaceButton extends PanelButton
   {

      private var _choice:FaceChoice;

      public function FaceButton(w:Number, h:Number)
      {
         super(w,h);
         name = "btn_face";
         this._choice = FaceChoice.shared();
         this._choice.addEventListener(Event.CHANGE,this.onChoice);
         LocalizationData.addCallback(this.onLabel,"st_srp_face");
         this.onChoice(null);
      }

      private function onLabel(value:String) : void
      {
         text = value;
      }

      // Shown with more than one face.
      private function onChoice(e:Event) : void
      {
         visible = this._choice.count > 1;
      }
   }
}
