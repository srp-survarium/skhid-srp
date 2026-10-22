package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.controls.TipRenderer;
   
   public class TextTipRendererSized extends TipRenderer
   {
      
      private const MAX_WIDTH:uint = 500;
      
      private const OFFSET_V:uint = 9;
      
      private const OFFSET_H:uint = 16;
      
      public var textField:TextField;
      
      public var background:MovieClip;
      
      public function TextTipRendererSized()
      {
         super();
         this.textField.styleSheet = MainScene.textStyleSheet;
      }
      
      override protected function invalidateData() : void
      {
         this.textField.wordWrap = true;
         this.textField.multiline = true;
         this.textField.width = this.MAX_WIDTH;
         this.textField.text = this.getTextLabel();
         this.textField.width = Math.min(this.textField.textWidth + 5,this.MAX_WIDTH);
         this.textField.height = this.textField.textHeight + 5;
         this.background.width = this.textField.textWidth + this.OFFSET_H * 2 + 3;
         this.background.height = this.textField.textHeight + this.OFFSET_V * 2 + 6;
      }
      
      protected function getTextLabel() : String
      {
         return _data.name;
      }
   }
}

