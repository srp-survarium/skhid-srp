package survarium.face
{
   import flash.events.Event;
   import survarium.controls.PanelButton;
   import survarium.data.LocalizationData;

   // The Face button (face-choice.md, Lobby): the player panel's, left of the Lobby button and as
   // it is (PanelButton), "FACE" (st_srp_face). Shown from the lobby's first frame, as the Lobby
   // button, and hidden when srp's account (FaceChoice) says the game data has one face. Its click
   // opens the faces' window (FaceWindow) as the Lobby button's opens the lobby window: the status
   // panel tells the lobby (StatusPanel.FACE_WINDOW).
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

      // Hidden with one face; shown before srp's account comes (the lobby is shown a moment before
      // it), so that it is there with the Lobby button.
      private function onChoice(e:Event) : void
      {
         visible = this._choice.count != 1;
      }
   }
}
