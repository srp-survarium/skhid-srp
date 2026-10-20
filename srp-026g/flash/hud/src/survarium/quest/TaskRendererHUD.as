package survarium.quest
{
   import flash.display.Sprite;
   import flash.text.StyleSheet;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.controls.ListItemRenderer;
   
   public class TaskRendererHUD extends ListItemRenderer
   {
      
      public var selectSprite:Sprite;
      
      public function TaskRendererHUD()
      {
         super();
      }
      
      override public function set data(data:Object) : void
      {
         super.data = data;
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         constraints.removeAllElements();
         constraintsDisabled = true;
         preventAutosizing = true;
         var style:StyleSheet = new StyleSheet();
         var styleObject:Object;
         styleObject = new Object();
         styleObject.color = "#5b6261";
         styleObject.size = 7;
         styleObject.fontSize = 7;
         style.setStyle("small",styleObject);
         textField.styleSheet = style;
      }
      
      override protected function updateText() : void
      {
         trace("TRH::",_label);
         if(_label)
         {
            visible = true;
            if(textField.text != _label)
            {
               textField.height = 28;
               textField.autoSize = TextFieldAutoSize.RIGHT;
               textField.text = _label as String;
               textField.y = (86 - textField.textHeight) / 2 - 5;
               this.selectSprite.height = textField.textHeight;
            }
         }
         else
         {
            visible = false;
         }
      }
   }
}

