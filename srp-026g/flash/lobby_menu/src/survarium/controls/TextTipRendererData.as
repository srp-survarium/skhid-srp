package survarium.controls
{
   public class TextTipRendererData extends TextTipRendererSized
   {
      
      public function TextTipRendererData()
      {
         super();
      }
      
      override protected function getTextLabel() : String
      {
         return _data.tipText;
      }
   }
}

