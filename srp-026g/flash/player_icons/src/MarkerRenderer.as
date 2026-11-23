package
{
   import survarium.constants.MarkerType;
   import survarium.controls.HUDMarker;
   import survarium.controls.SmokeMark;

   // A marker of the match's objects (create_player_icon's kind 1: a thrown grenade, a trap).
   public dynamic class MarkerRenderer extends HUDMarker
   {
      // The smoke grenade's mark shown in place of the icon, or null (another type: the icon's frame).
      protected var _smoke:SmokeMark = null;

      public function MarkerRenderer()
      {
         super();
      }

      override public function set type(value:uint) : void
      {
         if(this._smoke)
         {
            removeChild(this._smoke);
            _smoke = null;
         }
         var smoke:Boolean = value == MarkerType.SMOKE || value == MarkerType.TOXIC_SMOKE;
         this.icon.visible = !smoke;
         if(smoke)
         {
            _smoke = new SmokeMark(value == MarkerType.TOXIC_SMOKE);
            this._smoke.transform.matrix = this.icon.transform.matrix;
            addChildAt(this._smoke,getChildIndex(this.icon) + 1);
         }
         else
         {
            super.type = value;
         }
      }
   }
}
