package survarium.item
{
   import flash.display.Sprite;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ToolTip;
   import survarium.constants.Colors;
   import survarium.controls.IconRenderer;
   import survarium.core.Icon;
   
   public class ItemRendererUnlocked extends ToolTip
   {
      
      public var iconRenderer:IconRenderer;
      
      public var textField:TextField;
      
      public function ItemRendererUnlocked()
      {
         super();
      }
      
      public function set data(value:Object) : void
      {
         _data = value;
         invalidateData();
      }
      
      public function get data() : Object
      {
         return _data;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.iconRenderer.center = true;
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            if(_data)
            {
               visible = true;
               this.updateData();
            }
            else
            {
               visible = false;
               this.iconRenderer.content = null;
            }
         }
      }
      
      protected function updateData() : void
      {
         this.iconRenderer.content = _data.icon ? (_data.icon as Icon).clone() : null;
         this.textField.text = _data.name;
         this.textField.textColor = Colors.REGULAR;
      }
   }
}

