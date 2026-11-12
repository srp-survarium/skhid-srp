package
{
   import survarium.buttons.CheckBoxMultiline;
   
   public dynamic class CheckBoxSkinned extends CheckBoxMultiline
   {
      // The symbol has no art of a disabled box: its "disabled" and "selected_disabled" frames place the hover's
      // box (a faint tick), ticked or not. A disabled box shows its resting frame ("up": empty, "selected_up":
      // ticked), dimmed.
      private static const DISABLED_ALPHA:Number = 0.45;

      public function CheckBoxSkinned()
      {
         super();
         _stateMap.disabled = ["up"];
         addFrameScript(9,this.frame10,19,this.frame20,29,this.frame30,39,this.frame40,49,this.frame50,59,this.frame60,69,this.frame70,79,this.frame80);
      }

      override public function set enabled(value:Boolean) : void
      {
         super.enabled = value;
         alpha = value ? 1 : DISABLED_ALPHA;
      }

      internal function frame10() : *
      {
         stop();
      }
      
      internal function frame20() : *
      {
         stop();
      }
      
      internal function frame30() : *
      {
         stop();
      }
      
      internal function frame40() : *
      {
         stop();
      }
      
      internal function frame50() : *
      {
         stop();
      }
      
      internal function frame60() : *
      {
         stop();
      }
      
      internal function frame70() : *
      {
         stop();
      }
      
      internal function frame80() : *
      {
         stop();
      }
   }
}

