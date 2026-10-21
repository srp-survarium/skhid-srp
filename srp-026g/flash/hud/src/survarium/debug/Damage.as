package survarium.debug
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.layout.LayoutData;
   
   public class Damage extends MovieClip
   {
      
      public var layoutData:LayoutData;
      
      public var _prop1:DamageRender;
      
      public var _prop2:DamageRender;
      
      public var _prop3:DamageRender;
      
      public var _prop4:DamageRender;
      
      public var _prop5:DamageRender;
      
      public var _prop6:DamageRender;
      
      public var _prop7:DamageRender;
      
      public var _prop8:DamageRender;
      
      public var _prop9:DamageRender;
      
      public var _prop10:DamageRender;
      
      public var _prop11:DamageRender;
      
      public var _prop12:DamageRender;
      
      public var _prop13:DamageRender;
      
      public var _prop14:DamageRender;
      
      public var _prop15:DamageRender;
      
      public var _prop16:DamageRender;
      
      public var _prop17:DamageRender;
      
      public var _prop18:DamageRender;
      
      public var _prop19:DamageRender;
      
      public var _prop20:DamageRender;
      
      protected var newName:Array = ["test1","test2","test3","test4","test5","test6","test7","test8","test9","test10","test11","test12","test13","test14","test15","test16","test17","test18","test19","test20"];
      
      public function Damage()
      {
         super();
         this.fillNames();
         for(var i:int = 0; i < 20; i++)
         {
            if((this["_prop" + (i + 1).toString()]))
            {
               this["_prop" + (i + 1).toString()].visible = false;
            }
         }
      }
      
      public function set(part:int, val:Number, val_m:Number, affect:String) : void
      {
         var _pos:int = part;
         var _trg:DamageRender = this["_prop" + int(_pos + 1).toString()];
         if(!_trg)
         {
            trace("Damage::wrongId::",part);
            return;
         }
         if(!_trg.visible)
         {
            _trg.visible = true;
         }
         var _str:String = val.toFixed(3).toString() + "/" + val_m.toFixed(3).toString();
         if(_trg.textValue.text != _str)
         {
            _trg.textValue.text = _str;
         }
         var dmg:int = val / val_m * 510;
         var _color:uint = this.getColor(Math.min(510 - dmg,255),Math.min(dmg,255),0);
         if(_trg.textName.textColor != _color)
         {
            _trg.textName.textColor = _color;
         }
         var textStatus:TextField = _trg.textStatus;
         if(affect)
         {
            if(textStatus.textColor != _color)
            {
               textStatus.textColor = _color;
            }
            if(textStatus.text != affect)
            {
               textStatus.text = affect;
            }
         }
         else if(textStatus.text != "")
         {
            textStatus.text = "";
         }
      }
      
      public function setNames(value:Array) : void
      {
         newName = [];
         for(var i:int = 0; i < value.length; i++)
         {
            this.newName[value[i].index] = value[i].name;
         }
         this.sortItems();
         this.fillNames();
      }
      
      protected function fillNames() : void
      {
         for(var i:int = 0; i < this.newName.length; i++)
         {
            if((this["_prop" + (i + 1).toString()]))
            {
               this["_prop" + (i + 1).toString()]["textName"].text = this.newName[i];
            }
         }
      }
      
      protected function getColor(r:uint, g:uint, b:uint) : uint
      {
         var color:uint = r << 16 | g << 8 | b;
         return color;
      }
      
      protected function sortItems() : void
      {
         var stepX:uint = 220;
         var stepY:uint = 42;
         var maxL:uint = stepX * 5;
         var posX:uint = 0;
         var posY:uint = 0;
         for(var i:int = 0; i < 20; i++)
         {
            var renderer:MovieClip;
            renderer = this["_prop" + (i + 1).toString()];
            if(renderer)
            {
               renderer.x = posX;
               renderer.y = posY;
               posX += stepX;
               if(posX == maxL)
               {
                  posY += stepY;
                  posX = 0;
               }
            }
         }
      }
   }
}

