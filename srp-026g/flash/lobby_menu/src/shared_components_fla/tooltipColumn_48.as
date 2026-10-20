package shared_components_fla
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.gfx.TextFieldEx;
   
   public dynamic class tooltipColumn_48 extends MovieClip
   {
      
      public var textFieldRow1:TextField;
      
      public var textFieldRow2:TextField;
      
      public var textFieldLabel:TextField;
      
      public var textFieldRow3:TextField;
      
      public function tooltipColumn_48()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      internal function frame1() : *
      {
         TextFieldEx.setVerticalAlign(this.textFieldLabel,TextFieldEx.VALIGN_CENTER);
      }
   }
}

